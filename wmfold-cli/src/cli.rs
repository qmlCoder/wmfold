//! 机器接口（`wmfold -cli ...`）。
//!
//! 输出稳定、机器可读的 JSON，供桌面版通过 SSH 调用；也可以在服务器脚本里直接使用。
//! 协议版本见 [`PROTOCOL`]，桌面端会校验它。

use std::io::Write;

use serde::Serialize;

use wmfold_core::models::{FileEntry, MolStructure};
use wmfold_core::{chem, fs};

/// JSON 协议版本（字段不兼容变化时递增）。
pub const PROTOCOL: u32 = 1;

/// `list` 的返回结构。
#[derive(Debug, Serialize)]
pub struct ListResponse {
    pub protocol: u32,
    pub path: String,
    pub entries: Vec<FileEntry>,
}

/// `info` 的返回结构。
#[derive(Debug, Serialize)]
pub struct InfoResponse {
    pub protocol: u32,
    pub entry: FileEntry,
}

/// `structure` 的返回结构。
#[derive(Debug, Serialize)]
pub struct StructureResponse {
    pub protocol: u32,
    pub path: String,
    pub structure: MolStructure,
}

#[derive(Debug, Serialize)]
struct VersionResponse {
    protocol: u32,
    name: &'static str,
    version: &'static str,
}

/// 执行 CLI 子命令。
pub fn run(program: &str, args: Vec<String>) -> Result<(), String> {
    let mut it = args.into_iter();
    let Some(cmd) = it.next() else {
        return Err(usage(program));
    };
    let rest: Vec<String> = it.collect();
    match cmd.as_str() {
        "list" => {
            let (path, with_chem) = parse_path_chem(rest)?;
            let resp = list_response(&path, with_chem)?;
            print_json(&resp)
        }
        "info" => {
            let (path, with_chem) = parse_path_chem(rest)?;
            let resp = info_response(&path, with_chem)?;
            print_json(&resp)
        }
        "structure" => {
            let (path, _) = parse_path_chem(rest)?;
            let resp = structure_response(&path);
            print_json(&resp)
        }
        "version" => print_json(&VersionResponse {
            protocol: PROTOCOL,
            name: "wmfold-cli",
            version: env!("CARGO_PKG_VERSION"),
        }),
        "-h" | "--help" | "help" => {
            println!("{}", usage(program));
            Ok(())
        }
        other => Err(format!("unknown command '{other}'\n{}", usage(program))),
    }
}

pub fn usage(program: &str) -> String {
    format!(
        "{program} <command> [options]\n\
         \n\
         Commands:\n\
           list [PATH] [--chem]   List a directory as JSON (entries: name/path/size/times/kind/chem)\n\
           info <PATH> [--chem]   Print one file/directory as JSON\n\
           structure <PATH>       Print a molecule's 3D atoms/bonds as JSON (Å)\n\
           version                Print the CLI protocol version as JSON\n\
         \n\
         Options:\n\
           --chem                 Parse chemistry metadata for chemistry files\n\
           -h, --help             Show this help\n"
    )
}

/// 解析位置参数 + `--chem`。
fn parse_path_chem(args: Vec<String>) -> Result<(String, bool), String> {
    let mut path: Option<String> = None;
    let mut with_chem = false;
    for arg in args {
        match arg.as_str() {
            "--chem" | "-c" => with_chem = true,
            s if s.starts_with('-') => return Err(format!("unknown option '{s}'")),
            s => {
                if path.is_some() {
                    return Err("only one PATH is supported".to_string());
                }
                path = Some(s.to_string());
            }
        }
    }
    Ok((path.unwrap_or_else(|| ".".to_string()), with_chem))
}

fn list_response(path: &str, with_chem: bool) -> Result<ListResponse, String> {
    let mut entries = fs::list_dir(path)?;
    let absolute = std::path::absolute(path)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string());
    // 统一返回绝对路径，方便调用方（桌面端 / 脚本）继续导航
    for entry in entries.iter_mut() {
        entry.path = std::path::Path::new(&absolute)
            .join(&entry.name)
            .to_string_lossy()
            .to_string();
    }
    if with_chem {
        let paths: Vec<String> = entries
            .iter()
            .filter(|e| !e.is_dir && e.is_chem)
            .map(|e| e.path.clone())
            .collect();
        let metas = chem::read_chem_batch(&paths);
        for (p, meta) in paths.into_iter().zip(metas) {
            if let Some(entry) = entries.iter_mut().find(|e| e.path == p) {
                entry.chem = Some(meta);
            }
        }
    }
    Ok(ListResponse {
        protocol: PROTOCOL,
        path: absolute,
        entries,
    })
}

fn info_response(path: &str, with_chem: bool) -> Result<InfoResponse, String> {
    let mut entry = fs::stat(path)?;
    if with_chem && !entry.is_dir && entry.is_chem {
        entry.chem = Some(chem::read_chem(path));
    }
    Ok(InfoResponse {
        protocol: PROTOCOL,
        entry,
    })
}

/// 读取分子三维结构；失败时仍返回 `ok: false` 的合法 JSON，方便桌面端统一处理。
fn structure_response(path: &str) -> StructureResponse {
    StructureResponse {
        protocol: PROTOCOL,
        path: path.to_string(),
        structure: chem::read_structure(path),
    }
}

fn print_json<T: Serialize>(value: &T) -> Result<(), String> {
    let text = serde_json::to_string(value).map_err(|e| e.to_string())?;
    let mut out = std::io::stdout().lock();
    writeln!(out, "{text}")
        .and_then(|_| out.flush())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_path_and_chem_flag() {
        let (p, c) = parse_path_chem(vec!["/tmp".into(), "--chem".into()]).unwrap();
        assert_eq!(p, "/tmp");
        assert!(c);

        let (p, c) = parse_path_chem(vec![]).unwrap();
        assert_eq!(p, ".");
        assert!(!c);

        assert!(parse_path_chem(vec!["--bogus".into()]).is_err());
        assert!(parse_path_chem(vec!["a".into(), "b".into()]).is_err());
    }

    #[test]
    fn list_response_serializes_entries() {
        let resp = list_response(".", false).unwrap();
        assert_eq!(resp.protocol, PROTOCOL);
        assert!(!resp.entries.is_empty(), "当前目录不应为空");
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"entries\""));
        assert!(json.contains("\"isDir\""));
    }

    #[test]
    fn info_response_works_for_a_file() {
        let resp = info_response("Cargo.toml", false).unwrap();
        assert!(!resp.entry.is_dir);
        assert!(resp.entry.size > 0);
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"protocol\":1"));
    }

    #[test]
    fn usage_lists_commands() {
        let u = usage("wmfold");
        assert!(u.contains("list"));
        assert!(u.contains("info"));
        assert!(u.contains("structure"));
        assert!(u.contains("version"));
        assert!(u.starts_with("wmfold "));
    }

    #[test]
    fn structure_response_serializes_atoms() {
        let dir = std::path::Path::new("target").join("wmfold-cli-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("structure_{}.xyz", std::process::id()));
        std::fs::write(
            &path,
            "3\nwater\nO 0.0 0.0 0.0\nH 0.757 0.586 0.0\nH -0.757 0.586 0.0\n",
        )
        .unwrap();
        let resp = structure_response(&path.to_string_lossy());
        assert_eq!(resp.protocol, PROTOCOL);
        assert!(resp.structure.ok);
        assert_eq!(resp.structure.atoms.len(), 3);
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"atoms\""));
        assert!(json.contains("\"bonds\""));
        std::fs::remove_file(&path).ok();
    }
}
