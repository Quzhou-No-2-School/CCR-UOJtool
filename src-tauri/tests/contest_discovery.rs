//! 集成测试：比赛组装（计划 Task 6，使用 Task 3 夹具）。

mod common;

use common::build_fixture_contest;
use ccr_uojtool_lib::engine::ccr::{scan_contest, ScanOptions};
use ccr_uojtool_lib::engine::model::ProblemKind;
use std::path::Path;
use tempfile::TempDir;

fn opts() -> ScanOptions {
    ScanOptions {
        default_time: 1.0,
        default_mem: 128.0,
    }
}

#[test]
fn discovers_fixture_contest_in_ccr_order() {
    let tmp = TempDir::new().unwrap();
    build_fixture_contest(tmp.path());

    let contest = scan_contest(tmp.path(), &opts()).unwrap();
    let names: Vec<&str> = contest.problems.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["A", "B", "C", "D"]);
}

#[test]
fn problem_a_uniform_fulltext() {
    let tmp = TempDir::new().unwrap();
    build_fixture_contest(tmp.path());

    let contest = scan_contest(tmp.path(), &opts()).unwrap();
    let a = &contest.problems[0];
    assert_eq!(a.kind, ProblemKind::Traditional);
    assert_eq!(a.cases.len(), 10);
    assert!(a.uniform_score());
    assert_eq!(a.max_time(), 1.0);
    assert_eq!(a.max_mem(), 128.0);
    assert_eq!(a.checker.as_deref(), Some("fulltext"));
    assert!(a.prb_found);
    assert!(a.warnings.is_empty());
    // 测试点路径相对 data/A/，且按数字感知排序
    assert_eq!(a.cases[0].in_file, Path::new("A1.in"));
    assert_eq!(a.cases[9].in_file, Path::new("A10.in"));
    assert_eq!(a.cases[0].score, 10);
}

#[test]
fn problem_b_mixed_extensions_max_time() {
    let tmp = TempDir::new().unwrap();
    build_fixture_contest(tmp.path());

    let contest = scan_contest(tmp.path(), &opts()).unwrap();
    let b = &contest.problems[1];
    assert_eq!(b.cases.len(), 5);
    assert_eq!(b.max_time(), 2.0);
    assert_eq!(b.max_mem(), 512.0);
    assert_eq!(b.cases[0].in_file, Path::new("B1.inp"));
    assert_eq!(b.cases[0].out_file, Path::new("B1.ans"));
}

#[test]
fn problem_c_custom_checker_and_uneven_warning() {
    let tmp = TempDir::new().unwrap();
    build_fixture_contest(tmp.path());

    let contest = scan_contest(tmp.path(), &opts()).unwrap();
    let c = &contest.problems[2];
    assert_eq!(c.custom_checker.as_deref(), Some("checker.cpp"));
    assert!(!c.uniform_score());
    assert!(c.warnings.iter().any(|w| w.contains("非均匀")));
    assert_eq!(c.cases.len(), 2);
}

#[test]
fn problem_d_answers_only() {
    let tmp = TempDir::new().unwrap();
    build_fixture_contest(tmp.path());

    let contest = scan_contest(tmp.path(), &opts()).unwrap();
    let d = &contest.problems[3];
    assert_eq!(d.kind, ProblemKind::AnswersOnly);
}

#[test]
fn degrades_when_prb_missing() {
    let tmp = TempDir::new().unwrap();
    build_fixture_contest(tmp.path());
    std::fs::remove_file(tmp.path().join("data/C/.prb")).unwrap();

    let contest = scan_contest(tmp.path(), &opts()).unwrap();
    let c = &contest.problems[2];
    assert_eq!(c.kind, ProblemKind::Traditional);
    assert!(!c.prb_found);
    assert!(c.warnings.iter().any(|w| w.contains("未在 CCR 中配置")));
    // 点数按 discover 推断，限制用默认值
    assert_eq!(c.cases.len(), 2);
    assert_eq!(c.cases[0].time_limit, 1.0);
    assert_eq!(c.cases[0].memory_limit, 128.0);
    assert_eq!(c.cases[0].in_file, Path::new("C1.in"));
}

#[test]
fn missing_data_dir_is_error() {
    let tmp = TempDir::new().unwrap();
    assert!(scan_contest(tmp.path(), &opts()).is_err());
}

/// 恶意 .prb：测试点 in/out 含路径穿越 / 绝对路径 / 反斜杠 → 必须跳过并警告，
/// 不得把目录外文件（如 ../../secret.txt）带进导出数据包（终审 I1）。
#[test]
fn rejects_path_traversal_test_points() {
    let tmp = TempDir::new().unwrap();
    let data = tmp.path().join("data/M");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::write(data.join("good.in"), "in\n").unwrap();
    std::fs::write(data.join("good.out"), "out\n").unwrap();
    // 模拟目录外的敏感文件：非法点指向它时绝不能进入 cases
    std::fs::write(tmp.path().join("secret.txt"), "s3cret").unwrap();
    std::fs::write(
        data.join(".prb"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<problem type="TRA_0_4" maker="ccr-plus" version="1.1.0">
    <source dir="M" file="M" code="100"/>
    <task input="M.in" output="M.out" checker="fulltext" time="10">
        <subtask score="100">
            <point in="good.in" out="good.out" time="1" mem="128"/>
            <point in="../secret.txt" out="good.out" time="1" mem="128"/>
            <point in="/etc/passwd" out="good.out" time="1" mem="128"/>
            <point in="sub\x.in" out="good.out" time="1" mem="128"/>
        </subtask>
    </task>
</problem>
"#,
    )
    .unwrap();

    let contest = scan_contest(tmp.path(), &opts()).unwrap();
    let m = &contest.problems[0];
    assert_eq!(m.cases.len(), 1, "仅保留合法测试点");
    assert_eq!(m.cases[0].in_file, Path::new("good.in"));
    let illegal: Vec<_> = m
        .warnings
        .iter()
        .filter(|w| w.contains("文件名非法"))
        .collect();
    assert_eq!(illegal.len(), 3, "三个非法点各有警告，实际: {:?}", m.warnings);
}
