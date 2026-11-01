//! 测试点发现：目录内 in/out 文件配对 + 数字感知排序。
//!
//! 逐条对齐 CCR-Plus `Problem::getInAndOutFile()`（src/common/problem.cpp:400）：
//! - 对每个 (输入扩展名 a, 输出扩展名 b) 组合，扫描目录内所有文件；
//! - 文件名中出现 a / b 的位置把名字切成（前缀，后缀），两种命中累加到同一 key 计数表；
//! - 计数恰为 2 的 key 重构出一对文件名（前缀+a+后缀，前缀+b+后缀）；
//! - 选取配对数最多的组合，输入文件名按数字感知排序（QCollator numeric mode）。
//!
//! 与 CCR 的唯一偏差：原实现选组用 `ma - size <= 3` 容差从前往后扫，在纯
//! 空格形态（"2 in"/"2 out"）目录下会误选空组合；此处改为取最大组合。

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// 输入文件扩展名候选（按 CCR 顺序，无点形式按子串查找）。
pub const IN_EXTS: &[&str] = &[".in", ".inp", "in", "inp"];

/// 输出文件扩展名候选（按 CCR 顺序，无点形式按子串查找）。
pub const OUT_EXTS: &[&str] = &[
    ".out", ".ans", ".ou", ".an", ".sol", ".res", ".std", //
    "out", "ans", "ou", "an", "sol", "res", "std",
];

/// 数字感知比较：按 digit 段拆分，数字段按数值比较，其余按字节比较。
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (ab, bb) = (a.as_bytes(), b.as_bytes());
    let (mut i, mut j) = (0usize, 0usize);
    while i < ab.len() && j < bb.len() {
        let (ca, cb) = (ab[i], bb[j]);
        if ca.is_ascii_digit() && cb.is_ascii_digit() {
            let ia = i;
            while i < ab.len() && ab[i].is_ascii_digit() {
                i += 1;
            }
            let jb = j;
            while j < bb.len() && bb[j].is_ascii_digit() {
                j += 1;
            }
            let (da, db) = (&a[ia..i], &b[jb..j]);
            let (ta, tb) = (da.trim_start_matches('0'), db.trim_start_matches('0'));
            // 先比数值（位数，再逐位），数值相同则更短者（前导零更少）在前。
            let ord = ta
                .len()
                .cmp(&tb.len())
                .then_with(|| ta.cmp(tb))
                .then_with(|| da.len().cmp(&db.len()));
            if ord != Ordering::Equal {
                return ord;
            }
        } else if ca != cb {
            return ca.cmp(&cb);
        } else {
            i += 1;
            j += 1;
        }
    }
    // 一方耗尽：剩余少者在前（如 "A1" < "A10"）。
    (ab.len() - i).cmp(&(bb.len() - j))
}

/// 在文件名中按候选顺序查找扩展名，命中则返回去掉该段后的（前缀，后缀）。
fn classify(name: &str, exts: &[&str]) -> Option<(String, String)> {
    for ext in exts {
        if let Some(p) = name.find(ext) {
            return Some((name[..p].to_string(), name[p + ext.len()..].to_string()));
        }
    }
    None
}

/// 扫描目录，返回配对成功的测试点（已按输入文件名数字感知排序）与警告。
pub fn discover_cases(dir: &Path) -> (Vec<(PathBuf, PathBuf)>, Vec<String>) {
    let mut warnings = Vec::new();
    let mut names: Vec<String> = Vec::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            warnings.push(format!("无法读取目录 {}：{e}", dir.display()));
            return (Vec::new(), warnings);
        }
    };
    for ent in entries.flatten() {
        let path = ent.path();
        if !path.is_file() {
            continue;
        }
        if let Some(n) = path.file_name().and_then(|n| n.to_str()) {
            names.push(n.to_string());
        }
    }

    // 对每个 (a, b) 扩展名组合做 key 计数（BTreeMap 保持与 Qt QMap 一致的确定序）。
    let mut combos: Vec<Vec<Vec<(String, String)>>> = Vec::with_capacity(IN_EXTS.len());
    let mut max_pairs = 0usize;
    for a in IN_EXTS {
        let mut row = Vec::with_capacity(OUT_EXTS.len());
        for b in OUT_EXTS {
            let mut f: BTreeMap<(String, String), u32> = BTreeMap::new();
            for s in &names {
                if let Some(p) = s.find(a) {
                    *f.entry((s[..p].to_string(), s[p + a.len()..].to_string()))
                        .or_insert(0) += 1;
                }
                if let Some(q) = s.find(b) {
                    *f.entry((s[..q].to_string(), s[q + b.len()..].to_string()))
                        .or_insert(0) += 1;
                }
            }
            let mut cell = Vec::new();
            for (key, count) in f {
                if count == 2 {
                    let (prefix, suffix) = key;
                    let in_name = format!("{prefix}{a}{suffix}");
                    let out_name = format!("{prefix}{b}{suffix}");
                    if in_name != out_name {
                        cell.push((in_name, out_name));
                    }
                }
            }
            max_pairs = max_pairs.max(cell.len());
            row.push(cell);
        }
        combos.push(row);
    }

    // 选配对数最多的组合（并列取扩展名顺序最先者）。
    let mut res: Vec<(String, String)> = Vec::new();
    if max_pairs > 0 {
        'outer: for row in &combos {
            for cell in row {
                if cell.len() == max_pairs {
                    res = cell.clone();
                    break 'outer;
                }
            }
        }
    }
    res.sort_by(|x, y| natural_cmp(&x.0, &y.0));

    // 警告：目录中疑似 in/out 却未进入配对结果的文件（CCR 本身不警告，为报告需要补充）。
    let used: HashSet<&str> = res
        .iter()
        .flat_map(|(i, o)| [i.as_str(), o.as_str()])
        .collect();
    for s in &names {
        if used.contains(s.as_str()) {
            continue;
        }
        let is_in = classify(s, IN_EXTS).is_some();
        let is_out = classify(s, OUT_EXTS).is_some();
        if is_in && is_out {
            warnings.push(format!("{s} 既像输入又像输出文件，配对歧义，已跳过"));
        } else if is_in {
            warnings.push(format!("{s} 缺少配对的输出文件，已忽略"));
        } else if is_out {
            warnings.push(format!("{s} 缺少配对的输入文件，已忽略"));
        }
    }

    let pairs = res
        .into_iter()
        .map(|(i, o)| (dir.join(i), dir.join(o)))
        .collect();
    (pairs, warnings)
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
            .map(|(i, _)| i.file_name().unwrap().to_string_lossy().into_owned())
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
