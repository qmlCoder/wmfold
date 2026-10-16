//! 化学信息列的共享配置（对应原前端的 utils/chemColumns.ts）。

/// 单个化学信息列的展示配置
#[derive(Debug, Clone, Copy)]
pub struct ChemColDef {
    pub key: &'static str,
    pub label: &'static str,
    pub min_width: f32,
    pub sortable: bool,
    pub right: bool,
}

/// 各化学信息列的展示配置
pub const CHEM_DEFS: &[ChemColDef] = &[
    ChemColDef { key: "formula", label: "化学式", min_width: 96.0, sortable: true, right: false },
    ChemColDef { key: "nAtoms", label: "原子数", min_width: 60.0, sortable: true, right: true },
    ChemColDef { key: "charge", label: "电荷", min_width: 52.0, sortable: true, right: true },
    ChemColDef { key: "multip", label: "多重度", min_width: 60.0, sortable: true, right: true },
    ChemColDef { key: "nBasis", label: "基组函数", min_width: 70.0, sortable: true, right: true },
    ChemColDef { key: "molW", label: "分子量", min_width: 78.0, sortable: true, right: true },
    ChemColDef { key: "energy", label: "能量 (a.u.)", min_width: 86.0, sortable: true, right: true },
];

/// 按 key 查找列定义
pub fn chem_def(key: &str) -> Option<&'static ChemColDef> {
    CHEM_DEFS.iter().find(|d| d.key == key)
}

/// 「全部」默认展示的化学列
pub const CHEM_ALL: &[&str] = &["formula", "nAtoms", "charge", "multip", "molW", "energy"];

/// 每种具体类型可提供的化学列
pub fn chem_by_kind(kind: &str) -> &'static [&'static str] {
    match kind {
        "gjf" => &["formula", "nAtoms", "charge", "multip", "molW"],
        "log" => &["formula", "nAtoms", "charge", "multip", "energy", "molW"],
        "out" => &["formula", "nAtoms", "charge", "multip", "energy", "molW"],
        "fch" => &["formula", "nAtoms", "charge", "multip", "nBasis", "molW"],
        "inp" => &["formula", "nAtoms", "charge", "multip", "molW"],
        "mol" => &["formula", "nAtoms", "molW"],
        "sdf" => &["formula", "nAtoms", "molW"],
        "xyz" => &["formula", "nAtoms", "molW"],
        "molden" => &["formula", "nAtoms", "multip", "molW"],
        "dmo" => &["formula", "nAtoms", "molW"],
        "cub" => &["nAtoms"],
        _ => &[],
    }
}

/// 参与筛选/设置的类型代码（不含 全部/文件夹/化学文件）
pub const FILTER_KINDS: &[&str] = &[
    "gjf", "log", "out", "fch", "xyz", "mol", "sdf", "molden", "dmo", "inp", "cub",
];

/// 化学类型代码集合
pub fn is_chem_kind(kind: &str) -> bool {
    FILTER_KINDS.contains(&kind)
}

/// 类型筛选用文案
pub fn kind_label(kind: &str) -> &'static str {
    match kind {
        "gjf" => "高斯输入",
        "log" => "高斯日志",
        "out" => "量化输出",
        "fch" => "检查点",
        "xyz" => "XYZ",
        "mol" => "MOL",
        "sdf" => "SDF",
        "molden" => "Molden",
        "dmo" => "DMol",
        "inp" => "Orca 输入",
        "cub" => "格点",
        other => match chem_def(other) {
            Some(d) => d.label,
            None => "其他",
        },
    }
}
