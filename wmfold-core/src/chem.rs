//! 化学文件元数据读取
//!
//! 使用用户自研的波函数分析库 `rswfn` 对常见的化学文件进行快速解析，
//! 提取出化学式、原子数、电荷、多重度、基组函数数、分子量、能量等信息。
//!
//! 安全说明：rswfn 的某些读取器（`GjfReader`, `LogReader`）在遇到不完整的
//! 文件时会调用 `std::process::exit` 或 `panic!`。直接调用会拖垮整个应用，
//! 因此这里做了「预检（避免 exit）+ catch_unwind（拦截 panic）+ 结果降级」
//! 三层防护，任何失败都只影响单个文件的元数据，不影响资源管理器本身。

/// 化学解析缓存格式版本。
///
/// 修改 `ChemMeta` 字段、解析逻辑或 rswfn 读取行为时递增，旧缓存会自动失效。
pub const CHEM_CACHE_VERSION: u32 = 1;

use std::panic::AssertUnwindSafe;

use serde::Serialize;

use rswfn::base::{Atoms, Basis, Mole};
use rswfn::reader::{
    CubReader, DmoReader, FchReader, GjfReader, InpReader, LogReader, MdeReader, MolReader,
    OutReader, Reader, SdfReader, XyzReader,
};

use crate::models::{ChemMeta, MolAtom, MolStructure};

/// 扩展名到化学类型代码的映射
pub fn kind_from_ext(ext: &str) -> Option<&'static str> {
    match ext.to_lowercase().as_str() {
        "gjf" | "com" => Some("gjf"),
        "log" => Some("log"),
        "out" => Some("out"),
        "fch" | "fchk" => Some("fch"),
        "mol" => Some("mol"),
        "sdf" => Some("sdf"),
        "xyz" => Some("xyz"),
        "molden" | "mde" => Some("molden"),
        "dmo" | "dmi" => Some("dmo"),
        "inp" => Some("inp"),
        "cub" => Some("cub"),
        _ => None,
    }
}

/// 人类可读的文件类型名
fn type_label(kind: &str) -> String {
    match kind {
        "gjf" => "高斯输入文件".to_string(),
        "log" => "高斯日志文件".to_string(),
        "out" => "量化输出文件".to_string(),
        "fch" => "高斯检查点文件".to_string(),
        "mol" => "MDL 分子文件".to_string(),
        "sdf" => "SDF 结构文件".to_string(),
        "xyz" => "XYZ 坐标文件".to_string(),
        "molden" => "Molden 文件".to_string(),
        "dmo" => "DMol 文件".to_string(),
        "inp" => "Orca 输入文件".to_string(),
        "cub" => "格点文件".to_string(),
        _ => "化学文件".to_string(),
    }
}

/// 安全地读取文本文件的所有行，读取失败返回空。
fn read_file_lines(path: &str) -> Vec<String> {
    std::fs::read_to_string(path)
        .map(|s| s.lines().map(|l| l.to_string()).collect())
        .unwrap_or_default()
}

/// 文件内容是否包含某个关键字（用于区分读取器）
fn file_contains(path: &str, needle: &str) -> bool {
    read_file_lines(path)
        .iter()
        .any(|line| line.contains(needle))
}

/// 预检：判断文件是否适合调用对应的 rswfn 读取器，避免触发 `process::exit`。
fn precheck(path: &str, kind: &str) -> bool {
    match kind {
        // GjfReader::read_charge 在找不到「电荷 多重度」行时会 exit(0)。
        // 提前检查是否存在形如 `-?\d+  \d+` 的两整数行。
        "gjf" => read_file_lines(path).iter().any(|l| {
            let words: Vec<&str> = l.split_whitespace().collect();
            words.len() == 2
                && words.iter().all(|w| w.parse::<i64>().is_ok())
        }),
        // LogReader::get_coefs 在缺少 `pop=full` 时会 exit(0)。
        // 我们不调用 get_coefs，因此这里只需保证存在几何结构标记。
        "log" => {
            let lines = read_file_lines(path);
            lines.iter().any(|l| l.contains("Input orientation:"))
                || lines
                    .iter()
                    .any(|l| l.contains("Standard orientation:"))
        }
        _ => true,
    }
}

/// 用 rswfn 的 `LogReader::read_normalend` 判断日志是否正常结束。
///
/// `read_normalend` 直接取最后一行；空文件会越界 panic，因此仍走 catch_unwind。
fn read_log_normal_end(path: &str) -> Option<bool> {
    std::panic::catch_unwind(AssertUnwindSafe(|| {
        let reader = LogReader::new(path);
        reader.read_normalend()
    }))
    .ok()
}

/// 用 rswfn 读取「原子 + 基组」（不读取分子轨道系数，避免 exit）。
///
/// 返回 `Option<Some((atoms, basis))>`：
/// - `None`：完全失败（读取器构造/解析 panic 或预检不通过）。
fn read_atoms_basis(path: &str, kind: &str) -> Option<(Atoms, Option<Basis>)> {
    if !precheck(path, kind) {
        return None;
    }

    // 对于 .out，先判断是 Orca 还是 Gaussian 输出，以选择正确的读取器。
    let use_orca = kind == "out" && file_contains(path, "ORCA");
    let path2 = path.to_string();

    std::panic::catch_unwind(AssertUnwindSafe(move || {
        let reader: Box<dyn Reader> = match kind {
            "gjf" => Box::new(GjfReader::new(&path2)),
            "log" => Box::new(LogReader::new(&path2)),
            "out" => {
                if use_orca {
                    Box::new(OutReader::new(&path2))
                } else {
                    Box::new(LogReader::new(&path2))
                }
            }
            "fch" => Box::new(FchReader::new(&path2)),
            "mol" => Box::new(MolReader::new(&path2)),
            "sdf" => Box::new(SdfReader::new(&path2)),
            "xyz" => Box::new(XyzReader::new(&path2)),
            "molden" => Box::new(MdeReader::new(&path2)),
            "dmo" => Box::new(DmoReader::new(&path2)),
            "inp" => Box::new(InpReader::new(&path2)),
            "cub" => Box::new(CubReader::new(&path2)),
            _ => return None,
        };
        let atoms = reader.get_atoms();
        let basis = reader.get_basis();
        Some((atoms, basis))
    }))
    .ok()
    .flatten()
}

/// 尽力从文件原始文本中提取总能量（单位为 Hartree）。
fn extract_energy(path: &str) -> Option<f64> {
    let lines = read_file_lines(path);
    for line in &lines {
        // 高斯: " SCF Done:  E(RB3LYP) =  -76.4106001231     A.U. after    15 cycles"
        if let Some(e) = parse_after_equals(line, "SCF Done") {
            return Some(e);
        }
        // Orca: "FINAL SINGLE POINT ENERGY     -76.4106001231"
        if line.contains("FINAL SINGLE POINT ENERGY") {
            if let Some(v) = parse_number_in(line) {
                return Some(v);
            }
        }
    }
    None
}

/// 在包含关键字的行中，解析 `=  -xx.xxxx` 之后的数。
fn parse_after_equals(line: &str, keyword: &str) -> Option<f64> {
    if !line.contains(keyword) {
        return None;
    }
    // 查找到 "=" 后第一个浮点数
    let after = line.split('=').nth(1)?;
    parse_number_in(after)
}

/// 解析字符串中的第一个浮点数（允许科学计数法）。
fn parse_number_in(s: &str) -> Option<f64> {
    let re = regex::Regex::new(r"-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?").ok()?;
    let m = re.find(s)?;
    m.as_str().parse::<f64>().ok()
}

/// 标准原子量（g/mol）。未列出的元素返回其近似整数质量。
pub fn atomic_weight(sym: &str) -> f64 {
    match sym {
        "H" => 1.008,
        "D" => 2.014,
        "He" => 4.0026,
        "Li" => 6.94,
        "Be" => 9.0122,
        "B" => 10.81,
        "C" => 12.011,
        "N" => 14.007,
        "O" => 15.999,
        "F" => 18.998,
        "Ne" => 20.18,
        "Na" => 22.99,
        "Mg" => 24.305,
        "Al" => 26.982,
        "Si" => 28.085,
        "P" => 30.974,
        "S" => 32.06,
        "Cl" => 35.45,
        "Ar" => 39.948,
        "K" => 39.098,
        "Ca" => 40.078,
        "Sc" => 44.956,
        "Ti" => 47.867,
        "V" => 50.942,
        "Cr" => 51.996,
        "Mn" => 54.938,
        "Fe" => 55.845,
        "Co" => 58.933,
        "Ni" => 58.693,
        "Cu" => 63.546,
        "Zn" => 65.38,
        "Ga" => 69.723,
        "Ge" => 72.63,
        "As" => 74.922,
        "Se" => 78.971,
        "Br" => 79.904,
        "Kr" => 83.798,
        "Rb" => 85.468,
        "Sr" => 87.62,
        "Y" => 88.906,
        "Zr" => 91.224,
        "Nb" => 92.906,
        "Mo" => 95.95,
        "Tc" => 98.0,
        "Ru" => 101.07,
        "Rh" => 102.91,
        "Pd" => 106.42,
        "Ag" => 107.87,
        "Cd" => 112.41,
        "In" => 114.82,
        "Sn" => 118.71,
        "Sb" => 121.76,
        "Te" => 127.6,
        "I" => 126.9,
        "Xe" => 131.29,
        "Cs" => 132.91,
        "Ba" => 137.33,
        "La" => 138.91,
        "Ce" => 140.12,
        "Pr" => 140.91,
        "Nd" => 144.24,
        "Sm" => 150.36,
        "Eu" => 151.96,
        "Gd" => 157.25,
        "Tb" => 158.93,
        "Dy" => 162.5,
        "Ho" => 164.93,
        "Er" => 167.26,
        "Tm" => 168.93,
        "Yb" => 173.05,
        "Lu" => 174.97,
        "Hf" => 178.49,
        "Ta" => 180.95,
        "W" => 183.84,
        "Re" => 186.21,
        "Os" => 190.23,
        "Ir" => 192.22,
        "Pt" => 195.08,
        "Au" => 196.97,
        "Hg" => 200.59,
        "Tl" => 204.38,
        "Pb" => 207.2,
        "Bi" => 208.98,
        "Po" => 209.0,
        "At" => 210.0,
        "Rn" => 222.0,
        "Fr" => 223.0,
        "Ra" => 226.0,
        "Ac" => 227.0,
        "Th" => 232.04,
        "Pa" => 231.04,
        "U" => 238.03,
        "Np" => 237.0,
        "Pu" => 244.0,
        "Am" => 243.0,
        "Cm" => 247.0,
        "Bk" => 247.0,
        "Cf" => 251.0,
        "Es" => 252.0,
        "Fm" => 257.0,
        "Md" => 258.0,
        "No" => 259.0,
        "Lr" => 266.0,
        "Rf" => 267.0,
        "Db" => 268.0,
        "Sg" => 269.0,
        "Bh" => 270.0,
        "Hs" => 269.0,
        "Mt" => 278.0,
        "Ds" => 281.0,
        "Rg" => 282.0,
        "Cn" => 285.0,
        "Nh" => 286.0,
        "Fl" => 289.0,
        "Mc" => 290.0,
        "Lv" => 293.0,
        "Ts" => 294.0,
        "Og" => 294.0,
        other => other.parse::<f64>().unwrap_or(0.0),
    }
}

/// 将 rswfn 输出的化学式规范化为标准形式（去掉计数为 1 的数字）。
/// 例："H2O1" -> "H2O"。
fn normalize_formula(f: &str) -> String {
    let re = regex::Regex::new(r"([A-Z][a-z]?)(\d*)").unwrap();
    let mut out = String::new();
    for cap in re.captures_iter(f) {
        let sym = &cap[1];
        let count = cap[2].parse::<u64>().unwrap_or(1);
        out.push_str(sym);
        if count > 1 {
            out.push_str(&count.to_string());
        }
    }
    out
}

/// 判断一行是否为 Gaussian 路由段的分隔线（例如 ` -----`）。
fn is_dash_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.len() >= 3 && trimmed.chars().all(|c| c == '-')
}

/// 读取 Gaussian 作业关键词（route 段）。
///
/// - gjf：从第一个 `#` 行开始，读到空行为止，保留续行；
/// - log：从第一个 ` #` 行开始，读到 `-----` 分隔线为止。
///
/// 只做文本提取，不构造 rswfn 读取器，因此不会触发读取器的 panic / exit；
/// 返回前统一折叠空白，方便在预览面板中展示。
pub fn read_job(path: &str) -> Option<String> {
    let kind = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(kind_from_ext)
        .flatten()
        .unwrap_or("");
    if kind != "gjf" && kind != "log" {
        return None;
    }

    let lines = read_file_lines(path);
    let mut parts: Vec<String> = Vec::new();
    let mut in_route = false;
    for line in &lines {
        if kind == "gjf" {
            if !in_route {
                if line.trim_start().starts_with('#') {
                    in_route = true;
                    parts.push(line.trim().trim_start_matches('#').trim().to_string());
                }
            } else if line.trim().is_empty() {
                break;
            } else {
                parts.push(line.trim().trim_start_matches('#').trim().to_string());
            }
        } else if !in_route {
            if line.starts_with(" #") {
                in_route = true;
                parts.push(line.trim().trim_start_matches('#').trim().to_string());
            }
        } else if is_dash_line(line) {
            break;
        } else {
            parts.push(line.trim().trim_start_matches('#').trim().to_string());
        }
    }

    let job = parts.join(" ").split_whitespace().collect::<Vec<_>>().join(" ");
    if job.is_empty() {
        None
    } else {
        Some(job)
    }
}

/// 读取单个化学文件并返回完整元数据。
fn parse_chem(path: &str) -> ChemMeta {
    let kind = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(kind_from_ext)
        .flatten()
        .unwrap_or("");

    if kind.is_empty() {
        return ChemMeta {
            file_type: type_label(""),
            file_kind: String::new(),
            formula: String::new(),
            n_atoms: 0,
            charge: 0,
            multip: 0,
            n_alpha: 0,
            n_beta: 0,
            n_basis: 0,
            mol_w: 0.0,
            energy: None,
            job: None,
            normal_end: None,
            ok: false,
        };
    }

    // gjf / log 额外提取 Gaussian route 关键词（即使坐标解析失败也保留作业信息）
    let job = if kind == "gjf" || kind == "log" {
        read_job(path)
    } else {
        None
    };
    // log 额外读取是否正常结束（同样即使坐标解析失败也保留）
    let normal_end = if kind == "log" {
        read_log_normal_end(path)
    } else {
        None
    };

    let parsed = read_atoms_basis(path, kind);
    match parsed {
        Some((atoms, basis)) => {
            let mole = Mole::new(atoms, basis, None);
            let syms = mole.atoms().syms();
            let mol_w = syms.iter().map(|s| atomic_weight(s)).sum();
            let n_basis = mole
                .basis
                .as_ref()
                .map(|b| b.nbas())
                .unwrap_or(0);

            ChemMeta {
                file_type: type_label(kind),
                file_kind: kind.to_string(),
                formula: normalize_formula(&mole.formula()),
                n_atoms: mole.atoms().len(),
                charge: mole.charge(),
                multip: mole.multip(),
                n_alpha: mole.atoms().neles[0],
                n_beta: mole.atoms().neles[1],
                n_basis,
                mol_w,
                energy: extract_energy(path),
                job: job.clone(),
                normal_end,
                ok: true,
            }
        }
        None => {
            // 解析失败：至少给出类型与分子量（若能从文件名/兜底解析）。
            ChemMeta {
                file_type: type_label(kind),
                file_kind: kind.to_string(),
                formula: String::new(),
                n_atoms: 0,
                charge: 0,
                multip: 0,
                n_alpha: 0,
                n_beta: 0,
                n_basis: 0,
                mol_w: 0.0,
                energy: None,
                job,
                normal_end,
                ok: false,
            }
        }
    }
}

/// 批量读取化学文件元数据（并行）。
pub fn read_chem_batch(paths: &[String]) -> Vec<ChemMeta> {
    use rayon::prelude::*;
    paths.par_iter().map(|p| parse_chem(p)).collect()
}

/// 读取单个化学文件元数据。
pub fn read_chem(path: &str) -> ChemMeta {
    parse_chem(path)
}

/// 读取分子的三维结构（原子坐标 + 成键），供前端 three.js 预览。
///
/// 与 [`parse_chem`] 一样走 `read_atoms_basis` 的安全通道，任何解析失败都返回
/// `ok: false` 的空结构，不会让 rswfn 的 panic / exit 影响整个应用。
pub fn read_structure(path: &str) -> MolStructure {
    let kind = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(kind_from_ext)
        .flatten()
        .unwrap_or("");

    if kind.is_empty() {
        return MolStructure {
            ok: false,
            atoms: Vec::new(),
            bonds: Vec::new(),
        };
    }

    let Some((atoms, _basis)) = read_atoms_basis(path, kind) else {
        return MolStructure {
            ok: false,
            atoms: Vec::new(),
            bonds: Vec::new(),
        };
    };

    if atoms.len() == 0 {
        return MolStructure {
            ok: false,
            atoms: Vec::new(),
            bonds: Vec::new(),
        };
    }

    let syms = atoms.syms();
    let xyz_ang = atoms.xyzs_ang();
    let mol_atoms: Vec<MolAtom> = syms
        .iter()
        .zip(xyz_ang.iter())
        .map(|(element, [x, y, z])| MolAtom {
            element: element.clone(),
            x: *x,
            y: *y,
            z: *z,
        })
        .collect();

    // atoms.bonds 由 rswfn 在构建 Atoms 时按「距离 < 半径和 * 1.1」计算；
    // 少数读取器用 add() 逐个添加原子，此时 bonds 可能为空，这里补算一次。
    let mut bonds = atoms
        .bonds
        .lock()
        .map(|b| b.clone())
        .unwrap_or_default();
    if bonds.is_empty() && atoms.len() > 1 {
        let xyz_bohr = atoms.xyzs();
        let rads = atoms.rads();
        for i in 0..atoms.len() - 1 {
            for j in i + 1..atoms.len() {
                let [xi, yi, zi] = xyz_bohr[i];
                let [xj, yj, zj] = xyz_bohr[j];
                let dist =
                    ((xi - xj).powi(2) + (yi - yj).powi(2) + (zi - zj).powi(2)).sqrt();
                let limit = if rads[i] > 0.0 && rads[j] > 0.0 {
                    (rads[i] + rads[j]) * 1.1
                } else {
                    3.2
                };
                if dist < limit {
                    bonds.push([i, j]);
                }
            }
        }
    }

    MolStructure {
        ok: true,
        atoms: mol_atoms,
        bonds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNT: AtomicU32 = AtomicU32::new(0);

    fn tmp_file(ext: &str, content: &str) -> String {
        let mut p = std::env::temp_dir();
        let id = COUNT.fetch_add(1, Ordering::SeqCst);
        p.push(format!("wmfold_test_{}_{}.{}", std::process::id(), id, ext));
        std::fs::write(&p, content).unwrap();
        p.to_string_lossy().to_string()
    }

    #[test]
    fn xyz_water() {
        let path = tmp_file(
            "xyz",
            "3\nwater\nO 0.0 0.0 0.000\nH 0.757 0.586 0.000\nH -0.757 0.586 0.000\n",
        );
        let m = parse_chem(&path);
        assert!(m.ok, "解析 xyz 应成功");
        assert_eq!(m.formula, "H2O");
        assert_eq!(m.n_atoms, 3);
        assert_eq!(m.charge, 0);
        assert_eq!(m.multip, 1);
        assert!((m.mol_w - 18.015).abs() < 0.01);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn gjf_water() {
        let path = tmp_file(
            "gjf",
            "%chk=w.chk\n# b3lyp/6-31g(d) opt\n\nwater\n\n0 1\nO 0.000000 0.000000 0.000000\nH 0.757000 0.586000 0.000000\nH -0.757000 0.586000 0.000000\n\n",
        );
        let m = parse_chem(&path);
        assert!(m.ok, "解析 gjf 应成功");
        assert_eq!(m.formula, "H2O");
        assert_eq!(m.n_atoms, 3);
        assert_eq!(m.charge, 0);
        assert_eq!(m.multip, 1);
        assert_eq!(m.job.as_deref(), Some("b3lyp/6-31g(d) opt"));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn job_reads_gjf_and_log_routes() {
        let gjf = tmp_file(
            "gjf",
            "%chk=w.chk\n#p b3lyp/6-31g(d)\n opt freq\n\nwater\n\n0 1\nO 0.0 0.0 0.0\n",
        );
        assert_eq!(
            read_job(&gjf).as_deref(),
            Some("p b3lyp/6-31g(d) opt freq")
        );
        let log = tmp_file(
            "log",
            " #p b3lyp/6-31g(d) opt freq\n ------------\n SCF Done:  E(RB3LYP) =  -76.4\n",
        );
        assert_eq!(
            read_job(&log).as_deref(),
            Some("p b3lyp/6-31g(d) opt freq")
        );
        std::fs::remove_file(&gjf).ok();
        std::fs::remove_file(&log).ok();
    }

    #[test]
    fn mol_water() {
        let path = tmp_file(
            "mol",
            "water\n  JchemOpenBabel07251916162D\n\n  3  2  0  0  0  0  0  0  0  0999 V2000\n\n    0.0000    0.0000    0.0000 O   0  0  0  0  0  0  0  0  0  0  0  0\n",
        );
        // mol 文件中未给出电荷/多重度，默认中性单重态
        let m = parse_chem(&path);
        assert!(!m.ok, "不完整的 mol 不应被成功解析");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn log_normal_end_flag() {
        let ok = tmp_file(
            "log",
            " #p b3lyp/6-31g(d)\n SCF Done:  E(RB3LYP) =  -76.4\n Normal termination of Gaussian 16 at Thu Jan  1 00:00:00 2026.\n",
        );
        assert_eq!(read_log_normal_end(&ok), Some(true));

        let bad = tmp_file(
            "log",
            " #p b3lyp/6-31g(d)\n Error termination via Lnk1e at Thu Jan  1 00:00:00 2026.\n",
        );
        assert_eq!(read_log_normal_end(&bad), Some(false));

        let empty = tmp_file("log", "");
        assert_eq!(read_log_normal_end(&empty), None);

        std::fs::remove_file(&ok).ok();
        std::fs::remove_file(&bad).ok();
        std::fs::remove_file(&empty).ok();
    }

    #[test]
    fn energy_gaussian_log() {
        let path = tmp_file(
            "log",
            " SCF Done:  E(RB3LYP) =  -76.4106001231     A.U. after    15 cycles\n",
        );
        let e = extract_energy(&path);
        assert!(e.is_some());
        assert!((e.unwrap() - -76.4106001231).abs() < 1e-6);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn structure_xyz_water() {
        let path = tmp_file(
            "xyz",
            "3\nwater\nO 0.0 0.0 0.000\nH 0.757 0.586 0.000\nH -0.757 0.586 0.000\n",
        );
        let s = read_structure(&path);
        assert!(s.ok);
        assert_eq!(s.atoms.len(), 3);
        assert_eq!(s.atoms[0].element, "O");
        assert!((s.atoms[1].x - 0.757).abs() < 0.01);
        assert_eq!(s.bonds.len(), 2);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn structure_missing_file_is_not_ok() {
        let s = read_structure("E:\\definitely-not-a-real-wmfold-file.xyz");
        assert!(!s.ok);
        assert!(s.atoms.is_empty());
    }

    #[test]
    fn unknown_extension_is_not_chem() {
        assert_eq!(kind_from_ext("txt"), None);
        assert_eq!(kind_from_ext("gjf"), Some("gjf"));
        assert_eq!(kind_from_ext("fchk"), Some("fch"));
    }
}
