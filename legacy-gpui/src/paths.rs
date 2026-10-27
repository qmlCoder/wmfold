//! Windows 路径解析辅助（对应原前端的 utils/path.ts）。

/// 虚拟根，"此电脑" 视图
pub const THIS_PC: &str = "::ThisPC";

/// 是否为 "此电脑" 虚拟路径
pub fn is_this_pc(p: &str) -> bool {
    p == THIS_PC
}

/// 是否为盘符（如 "C:"）
fn is_drive(seg: &str) -> bool {
    let b = seg.as_bytes();
    b.len() == 2 && b[1] == b':' && b[0].is_ascii_alphabetic()
}

/// 规范化分隔符后拆分为段
pub fn to_segs(p: &str) -> Vec<String> {
    p.replace('\\', "/")
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// 拼接完整 Windows 路径
pub fn join_path(segs: &[String]) -> String {
    if segs.is_empty() {
        return String::new();
    }
    // 盘符根，如 "C:"
    if segs.len() == 1 && is_drive(&segs[0]) {
        return format!("{}\\", segs[0]);
    }
    segs.join("\\")
}

/// 获取父路径
pub fn parent_path(p: &str) -> String {
    if is_this_pc(p) {
        return THIS_PC.to_string();
    }
    let mut segs = to_segs(p);
    if segs.is_empty() {
        return THIS_PC.to_string();
    }
    if segs.len() == 1 && is_drive(&segs[0]) {
        return THIS_PC.to_string();
    }
    segs.pop();
    join_path(&segs)
}

/// 添加子路径
pub fn child_path(p: &str, name: &str) -> String {
    if is_this_pc(p) {
        return format!("{}\\", name);
    }
    let mut segs = to_segs(p);
    segs.push(name.to_string());
    join_path(&segs)
}

/// 面包屑各节点的可导航路径：`(label, path)`
pub fn breadcrumb_paths(p: &str) -> Vec<(String, String)> {
    let mut crumbs = vec![("此电脑".to_string(), THIS_PC.to_string())];
    if is_this_pc(p) {
        return crumbs;
    }
    let segs = to_segs(p);
    if segs.is_empty() {
        return crumbs;
    }

    let mut acc: Vec<String> = Vec::new();
    if is_drive(&segs[0]) {
        acc.push(segs[0].clone());
        crumbs.push((segs[0].clone(), join_path(&acc)));
        for s in &segs[1..] {
            acc.push(s.clone());
            crumbs.push((s.clone(), join_path(&acc)));
        }
    } else {
        for s in &segs {
            acc.push(s.clone());
            crumbs.push((s.clone(), join_path(&acc)));
        }
    }
    crumbs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crumbs_and_navigation() {
        assert_eq!(parent_path("C:\\Users\\me"), "C:\\Users");
        assert_eq!(parent_path("C:\\"), THIS_PC);
        assert_eq!(child_path("C:\\Users", "me"), "C:\\Users\\me");
        assert_eq!(child_path(THIS_PC, "C:"), "C:\\");
        let c = breadcrumb_paths("C:\\Users\\me");
        assert_eq!(c.len(), 4);
        assert_eq!(c[0].1, THIS_PC);
        assert_eq!(c[1].0, "C:");
        assert_eq!(c[2].1, "C:\\Users");
        assert_eq!(c[3].1, "C:\\Users\\me");
    }
}
