//! 格式化与图标等展示辅助函数（对应原前端的 utils/format.ts）。

use gpui_kit::assets::IconName;
use gpui_kit::{rgb, Hsla};

use crate::models::FileEntry;

fn hex(c: u32) -> Hsla {
    rgb(c).into()
}

/// 字节数格式化为可读字符串
pub fn format_size(bytes: u64) -> String {
    if bytes == 0 {
        return "—".to_string();
    }
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut v = bytes as f64;
    let mut i = 0usize;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{} {}", v.round() as u64, UNITS[i])
    } else {
        format!("{:.1} {}", v, UNITS[i])
    }
}

/// 毫秒时间戳格式化为 "yyyy/MM/dd HH:mm"
pub fn format_date(ms: Option<u64>) -> String {
    use chrono::{Local, TimeZone};
    match ms {
        None => "—".to_string(),
        Some(ms) => match Local.timestamp_millis_opt(ms as i64).single() {
            Some(dt) => dt.format("%Y/%m/%d %H:%M").to_string(),
            None => "—".to_string(),
        },
    }
}

/// 化学类型代码 → 简短标签
pub fn kind_short(kind: &str) -> &'static str {
    match kind {
        "gjf" => "GJF",
        "log" => "LOG",
        "out" => "OUT",
        "fch" => "FCHK",
        "mol" => "MOL",
        "sdf" => "SDF",
        "xyz" => "XYZ",
        "molden" => "MOLDEN",
        "dmo" => "DMO",
        "inp" => "INP",
        "cub" => "CUB",
        _ => "",
    }
}

/// 化学文件类型配色
pub fn chem_color(kind: &str) -> Hsla {
    match kind {
        "gjf" => hex(0x409EFF),
        "log" => hex(0x67C23A),
        "out" => hex(0xE6A23C),
        "fch" => hex(0xF56C6C),
        "mol" => hex(0x909399),
        "sdf" => hex(0x909399),
        "xyz" => hex(0x7A9E4E),
        "molden" => hex(0x8B5CF6),
        "dmo" => hex(0x0BB3A7),
        "inp" => hex(0xFF8C00),
        "cub" => hex(0x3B82F6),
        _ => hex(0x909399),
    }
}

/// 化学类型代码 → 图标
pub fn chem_icon(kind: &str) -> IconName {
    match kind {
        "cub" => IconName::FileSpreadsheet,
        _ => IconName::FileText,
    }
}

/// 文件扩展名 → 图标
pub fn file_icon(entry: &FileEntry) -> IconName {
    if entry.is_dir {
        return IconName::Folder;
    }
    match entry.extension.as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "svg" => IconName::Image,
        "mp3" | "wav" | "flac" | "m4a" => IconName::Headphones,
        "mp4" | "avi" | "mkv" | "mov" | "wmv" => IconName::Video,
        "zip" | "rar" | "7z" | "gz" | "tar" => IconName::Archive,
        "exe" | "dll" | "msi" => IconName::Cpu,
        "py" | "rs" | "js" | "ts" | "c" | "cpp" | "h" => IconName::FileCode,
        "xls" | "xlsx" | "csv" => IconName::FileSpreadsheet,
        "ppt" | "pptx" => IconName::Presentation,
        "doc" | "docx" | "md" | "txt" | "pdf" => IconName::FileText,
        "db" | "sqlite" => IconName::Database,
        "json" => IconName::Braces,
        _ => {
            if entry.is_chem {
                chem_icon(&entry.kind)
            } else {
                IconName::File
            }
        }
    }
}

/// 根据文件类型返回描述文本
pub fn type_label(entry: &FileEntry) -> String {
    if entry.is_dir {
        return "文件夹".to_string();
    }
    match entry.kind.as_str() {
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
        "" => "文件".to_string(),
        _ => format!("{} 文件", entry.extension.to_uppercase()),
    }
}

/// 自然排序比较（file2 在 file10 之前）
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let a = a.to_lowercase();
    let b = b.to_lowercase();
    let (mut ai, mut bi) = (0usize, 0usize);
    let ab = a.as_bytes();
    let bb = b.as_bytes();

    while ai < ab.len() && bi < bb.len() {
        let ca = ab[ai];
        let cb = bb[bi];
        if ca.is_ascii_digit() && cb.is_ascii_digit() {
            let (na, na_end) = read_number(ab, ai);
            let (nb, nb_end) = read_number(bb, bi);
            match na.cmp(&nb) {
                Ordering::Equal => {
                    ai = na_end;
                    bi = nb_end;
                }
                other => return other,
            }
        } else {
            match ca.cmp(&cb) {
                Ordering::Equal => {
                    ai += 1;
                    bi += 1;
                }
                other => return other,
            }
        }
    }
    (ab.len() - ai).cmp(&(bb.len() - bi))
}

/// 从字节串中读出一段数字，返回 `(数值, 结束下标)`。
fn read_number(bytes: &[u8], start: usize) -> (u128, usize) {
    let mut i = start;
    let mut v: u128 = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        v = v.saturating_mul(10).saturating_add((bytes[i] - b'0') as u128);
        i += 1;
    }
    (v, i)
}
