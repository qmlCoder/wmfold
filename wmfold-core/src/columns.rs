//! 化学列定义与各文件类型可用列。
//!
//! 前端（Vue）有自己的表格，但「某种类型能显示哪些列」这条规则
//! 必须一致；后端校验设置时从这里取。

/// 单个化学信息列的元数据（与前端 `utils/chemColumns.ts` / GPUI 版一致）。
#[derive(Debug, Clone, Copy)]
pub struct ChemColDef {
    /// 列 key，同时用于 JSON 设置文件
    pub key: &'static str,
    /// 中文标签（设置界面用）
    pub label: &'static str,
    /// 设置持久化 / 校验使用的列键（保持 ASCII）
    pub short_label: &'static str,
    /// 该列是否右对齐（数值列）
    pub right: bool,
}

/// 全部化学信息列。
pub const CHEM_DEFS: &[ChemColDef] = &[
    ChemColDef { key: "formula", label: "化学式", short_label: "Formula", right: false },
    ChemColDef { key: "nAtoms", label: "原子数", short_label: "Atoms", right: true },
    ChemColDef { key: "charge", label: "电荷", short_label: "Charge", right: true },
    ChemColDef { key: "multip", label: "多重度", short_label: "Mult", right: true },
    ChemColDef { key: "nBasis", label: "基组函数", short_label: "Basis", right: true },
    ChemColDef { key: "molW", label: "分子量", short_label: "MW", right: true },
    ChemColDef { key: "energy", label: "能量 (a.u.)", short_label: "Energy", right: true },
    ChemColDef { key: "job", label: "作业", short_label: "Job", right: false },
    ChemColDef { key: "normalEnd", label: "正常结束", short_label: "NormalEnd", right: false },
];

/// 按 key 查找列定义。
pub fn chem_def(key: &str) -> Option<&'static ChemColDef> {
    CHEM_DEFS.iter().find(|d| d.key == key)
}

/// 参与筛选 / 设置的类型代码。
pub const FILTER_KINDS: &[&str] = &[
    "gjf", "log", "out", "fch", "xyz", "mol", "sdf", "molden", "dmo", "inp", "cub",
];

/// 某种类型可提供的化学列。
pub fn available_columns(kind: &str) -> &'static [&'static str] {
    match kind {
        "gjf" => &["formula", "nAtoms", "charge", "multip", "molW", "job"],
        "log" => &["formula", "nAtoms", "charge", "multip", "energy", "molW", "job", "normalEnd"],
        "out" => &["formula", "nAtoms", "charge", "multip", "energy", "molW"],
        "fch" => &["formula", "nAtoms", "charge", "multip", "nBasis", "molW"],
        "inp" => &["formula", "nAtoms", "charge", "multip", "molW"],
        "mol" | "sdf" | "xyz" | "dmo" => &["formula", "nAtoms", "molW"],
        "molden" => &["formula", "nAtoms", "multip", "molW"],
        "cub" => &["nAtoms"],
        _ => &[],
    }
}

/// 是否为已知的化学类型代码。
pub fn is_chem_kind(kind: &str) -> bool {
    FILTER_KINDS.contains(&kind)
}

/// 类型的简短显示名（ASCII）。
pub fn kind_code(kind: &str) -> &'static str {
    match kind {
        "gjf" => "GJF",
        "log" => "LOG",
        "out" => "OUT",
        "fch" => "FCH",
        "xyz" => "XYZ",
        "mol" => "MOL",
        "sdf" => "SDF",
        "molden" => "MOLDEN",
        "dmo" => "DMO",
        "inp" => "INP",
        "cub" => "CUB",
        _ => "FILE",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_columns_and_code() {
        for kind in FILTER_KINDS {
            assert!(!available_columns(kind).is_empty(), "类型 {kind} 缺少列");
            assert_ne!(kind_code(kind), "FILE");
            for col in available_columns(kind) {
                assert!(chem_def(col).is_some(), "未知列 {col}");
            }
        }
    }

    #[test]
    fn unknown_kind_is_empty() {
        assert!(available_columns("txt").is_empty());
        assert!(!is_chem_kind("txt"));
        assert_eq!(kind_code("txt"), "FILE");
    }
}
