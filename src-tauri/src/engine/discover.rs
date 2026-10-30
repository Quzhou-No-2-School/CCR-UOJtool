//! 测试点发现：目录内 in/out 文件配对 + 数字感知排序。
//!
//! 对齐 CCR-Plus `getInAndOutFile()` 语义（计划 0.3）：
//! - 扩展名候选按顺序做"子串查找"，首个命中者生效；
//! - 去掉扩展名后的（前缀，后缀）相同的 in/out 文件配成一对；
//! - 仅当某 key 恰有一个 in 文件且恰有一个 out 文件时才收为测试点。

use std::cmp::Ordering;
use std::path::Path;

/// 输入文件扩展名候选（按 CCR 顺序，无点形式按子串查找）。
pub const IN_EXTS: &[&str] = &[".in", ".inp", "in", "inp"];

/// 输出文件扩展名候选（按 CCR 顺序，无点形式按子串查找）。
pub const OUT_EXTS: &[&str] = &[
    ".out", ".ans", ".ou", ".an", ".sol", ".res", ".std", //
    "out", "ans", "ou", "an", "sol", "res", "std",
];

/// 数字感知比较：按 digit 段拆分，数字段按数值比较，其余按字节比较。
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    todo!()
}

/// 扫描目录，返回配对成功的测试点（已按输入文件名数字感知排序）与警告。
pub fn discover_cases(dir: &Path) -> (Vec<(std::path::PathBuf, std::path::PathBuf)>, Vec<String>) {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn names(pairs: &[(PathBuf, PathBuf)]) -> Vec<String> {
        pairs
            .iter()
            .map(|(i, _)| {
                i.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    #[test]
    fn natural_sort_digit_runs() {
        assert_eq!(natural_cmp("C2", "C10"), Ordering::Less);
        assert_eq!(natural_cmp("C10", "C2"), Ordering::Greater);
        assert_eq!(natural_cmp("A1", "A10"), Ordering::Less);
        assert_eq!(natural_cmp("9", "10"), Ordering::Less);
        assert_eq!(natural_cmp("abc", "abd"), Ordering::Less);
        assert_eq!(natural_cmp("A1", "A1"), Ordering::Equal);
    }

    #[test]
    fn pairs_basic_in_out() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        fs::write(dir.join("A1.in"), "in").unwrap();
        fs::write(dir.join("A1.out"), "out").unwrap();

        let (pairs, warnings) = discover_cases(dir);
        assert_eq!(pairs, vec![(dir.join("A1.in"), dir.join("A1.out"))]);
        assert!(warnings.is_empty());
    }

    #[test]
    fn pairs_mixed_extensions() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        fs::write(dir.join("B1.inp"), "in").unwrap();
        fs::write(dir.join("B1.ans"), "out").unwrap();

        let (pairs, warnings) = discover_cases(dir);
        assert_eq!(pairs, vec![(dir.join("B1.inp"), dir.join("B1.ans"))]);
        assert!(warnings.is_empty());
    }

    #[test]
    fn sorts_numerically_not_lexically() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        for i in [10usize, 2, 1] {
            fs::write(dir.join(format!("C{i}.in")), "in").unwrap();
            fs::write(dir.join(format!("C{i}.out")), "out").unwrap();
        }

        let (pairs, _) = discover_cases(dir);
        assert_eq!(names(&pairs), vec!["C1.in", "C2.in", "C10.in"]);
    }

    #[test]
    fn input_without_output_warns() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        fs::write(dir.join("lonely.in"), "in").unwrap();

        let (pairs, warnings) = discover_cases(dir);
        assert!(pairs.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("lonely.in"));
    }

    #[test]
    fn ignores_unrelated_files() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        fs::write(dir.join("D1.in"), "in").unwrap();
        fs::write(dir.join("D1.out"), "out").unwrap();
        fs::write(dir.join("std.cpp"), "// std").unwrap();
        fs::write(dir.join(".prb"), "<xml/>").unwrap();
        fs::write(dir.join("checker.cpp"), "// checker").unwrap();

        let (pairs, _) = discover_cases(dir);
        assert_eq!(pairs, vec![(dir.join("D1.in"), dir.join("D1.out"))]);
    }

    #[test]
    fn pairs_extensionless_space_form() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        fs::write(dir.join("2 in"), "in").unwrap();
        fs::write(dir.join("2 out"), "out").unwrap();

        let (pairs, warnings) = discover_cases(dir);
        assert_eq!(pairs, vec![(dir.join("2 in"), dir.join("2 out"))]);
        assert!(warnings.is_empty());
    }
}
