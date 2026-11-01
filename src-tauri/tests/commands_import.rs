//! 集成测试：比赛 zip 导入（解压 + 定位比赛根目录）。

mod common;

use ccr_uojtool_lib::commands::import_zip_to_dir;
use ccr_uojtool_lib::engine::ccr::{ScanOptions, scan_contest};
use common::build_fixture_contest;
use std::path::Path;
use tempfile::TempDir;

/// 把目录打成 zip（保留相对路径）。
fn zip_dir(src: &Path, zip_path: &Path, wrap_in_folder: Option<&str>) {
    let file = std::fs::File::create(zip_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    fn walk(zip: &mut zip::ZipWriter<std::fs::File>, dir: &Path, prefix: &str) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
            if path.is_dir() {
                walk(zip, &path, &format!("{name}/"));
            } else {
                zip.start_file(&name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                std::io::copy(&mut std::fs::File::open(&path).unwrap(), zip).unwrap();
            }
        }
    }
    let prefix = wrap_in_folder.map(|f| format!("{f}/")).unwrap_or_default();
    walk(&mut zip, src, &prefix);
    zip.finish().unwrap();
}

/// 导入后的目录能被 scan_contest 识别出 A/B/C/D 四题。
fn assert_scannable(contest_root: &Path) {
    let contest = scan_contest(contest_root, &ScanOptions::default()).unwrap();
    let names: Vec<&str> = contest.problems.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["A", "B", "C", "D"]);
}

#[test]
fn import_zip_with_contest_at_root() {
    let tmp = TempDir::new().unwrap();
    build_fixture_contest(tmp.path());
    let zip_path = tmp.path().join("contest.zip");
    zip_dir(tmp.path(), &zip_path, None);

    let dest = TempDir::new().unwrap();
    let root = import_zip_to_dir(&zip_path, dest.path()).unwrap();
    assert_eq!(root, dest.path());
    assert_scannable(&root);
}

#[test]
fn import_zip_with_single_folder() {
    let tmp = TempDir::new().unwrap();
    build_fixture_contest(tmp.path());
    let zip_path = tmp.path().join("contest.zip");
    zip_dir(tmp.path(), &zip_path, Some("my-contest"));

    let dest = TempDir::new().unwrap();
    let root = import_zip_to_dir(&zip_path, dest.path()).unwrap();
    assert_eq!(root, dest.path().join("my-contest"));
    assert_scannable(&root);
}

#[test]
fn import_missing_zip_is_error() {
    let tmp = TempDir::new().unwrap();
    assert!(import_zip_to_dir(&tmp.path().join("nope.zip"), &tmp.path().join("x")).is_err());
}
