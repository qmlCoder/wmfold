use serde::{Deserialize, Serialize};

/// 化学文件元数据
///
/// 通过 rswfn 库对常见化学文件（gjf/log/fch/mol/sdf/xyz/molden/dmo/inp/cub）
/// 进行快速解析后得到的结构化信息，在资源管理器表头中显示。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChemMeta {
    /// 人类可读的文件类型描述，如 "高斯输入文件"
    pub file_type: String,
    /// 短类型代码，如 "gjf"
    pub file_kind: String,
    /// 化学式，如 "H2O"
    pub formula: String,
    /// 原子数量
    pub n_atoms: usize,
    /// 分子总电荷
    pub charge: isize,
    /// 自旋多重度
    pub multip: usize,
    /// α 电子数
    pub n_alpha: usize,
    /// β 电子数
    pub n_beta: usize,
    /// 基组函数（原子轨道）数量，无基组时为 0
    pub n_basis: usize,
    /// 分子量（g/mol），无原子时取 0
    pub mol_w: f64,
    /// 从输出文件中尽力提取的总能量（Hartree）
    pub energy: Option<f64>,
    /// gjf / log 的作业关键词（Gaussian route，例如 `p b3lyp/6-31g(d) opt freq`）
    #[serde(default)]
    pub job: Option<String>,
    /// log 文件是否正常结束（rswfn `LogReader::read_normalend`）
    #[serde(default)]
    pub normal_end: Option<bool>,
    /// 是否成功解析（true 表示确实从文件中读出了分子信息）
    pub ok: bool,
}

/// 单个原子的三维坐标（单位：Å）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MolAtom {
    /// 元素符号，例如 "C" / "H" / "O"
    pub element: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// 分子三维结构，供前端 three.js 预览使用
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MolStructure {
    /// 是否成功读出原子坐标
    pub ok: bool,
    /// 原子列表（单位 Å）
    pub atoms: Vec<MolAtom>,
    /// 成键原子索引对；索引对应 `atoms`
    pub bonds: Vec<[usize; 2]>,
}

/// 目录/文件条目
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub created: Option<u64>,
    pub modified: Option<u64>,
    /// 小写扩展名（不含点），目录为空字符串
    pub extension: String,
    /// 化学类型代码（"gjf" 等），非化学文件为空
    pub kind: String,
    pub is_chem: bool,
    pub is_hidden: bool,
    pub is_symlink: bool,
    /// 化学元数据（仅在调用读取命令后填充；列表命令不填充以保持快速）
    pub chem: Option<ChemMeta>,
}

/// 磁盘信息
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveInfo {
    pub letter: String,
    pub label: String,
    pub total: Option<u64>,
    pub free: Option<u64>,
}

/// 快速访问项（例如桌面、文档等）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickAccess {
    pub name: String,
    pub path: String,
    pub icon: String,
    /// 是否为用户自行固定（右键“添加到快速访问”）的目录；
    /// 内置项（桌面 / 文档 / 下载…）为 false，前端据此显示“取消固定”。
    pub pinned: bool,
}
