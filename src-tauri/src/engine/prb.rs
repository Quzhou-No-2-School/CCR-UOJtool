//! `.prb` 题目配置解析（CCR-Plus `SaveConfiguration()` 生成的 XML，见计划 0.2）。

use crate::engine::model::ProblemKind;
use std::path::Path;

/// `.prb` 解析结果。
#[derive(Debug, Clone)]
pub struct PrbConfig {
    pub kind: ProblemKind,
    /// `<source dir>`：选手源码子目录名。
    pub source_dir: Option<String>,
    /// `<task input>`。
    pub input: Option<String>,
    /// `<task output>`。
    pub output: Option<String>,
    /// `<task checker>`（已去扩展名，如 "fulltext"）。
    pub checker: Option<String>,
    /// (subtask 分值, 测试点列表)。
    pub subtasks: Vec<(u32, Vec<PointCfg>)>,
}

/// 单个 `<point>`。
#[derive(Debug, Clone)]
pub struct PointCfg {
    pub in_file: String,
    pub out_file: String,
    /// 每点独立时限（秒，浮点）。
    pub time: Option<f64>,
    /// 每点独立内存（MB，浮点）。
    pub mem: Option<f64>,
    /// ANS 题的提交文件名（若有）。
    pub submit: Option<String>,
}

/// 与 CCR `RemoveFileExtension` 一致：只去最后一个 `.` 及其后内容。
pub fn strip_extension(s: &str) -> String {
    match s.rsplit_once('.') {
        Some((stem, _)) => stem.to_string(),
        None => s.to_string(),
    }
}

/// 根节点 `type` 属性 → 题目类型（计划 0.2 映射表）。
fn parse_kind(type_attr: Option<&str>) -> ProblemKind {
    match type_attr {
        Some("TRA") | Some("TRA_0_4") => ProblemKind::Traditional,
        Some("ANS") | Some("ANS_0_4") => ProblemKind::AnswersOnly,
        Some("INT") | Some("INT_0_4") => ProblemKind::Interactive,
        _ => ProblemKind::Unknown,
    }
}

/// 解析 `.prb` 文件；文件不存在、XML 损坏或根节点非 `<problem>` 时返回 Err。
pub fn parse_prb(path: &Path) -> Result<PrbConfig, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("读取 {} 失败：{e}", path.display()))?;
    parse_prb_str(&text).map_err(|e| format!("解析 {} 失败：{e}", path.display()))
}

/// 解析 `.prb` XML 字符串。
fn parse_prb_str(xml: &str) -> Result<PrbConfig, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("XML 语法错误：{e}"))?;
    let root = doc.root_element();
    if root.tag_name().name() != "problem" {
        return Err(format!(
            "根节点为 <{}>，期望 <problem>",
            root.tag_name().name()
        ));
    }

    let kind = parse_kind(root.attribute("type"));

    let source_dir = root
        .children()
        .filter(|n| n.is_element())
        .find(|n| n.tag_name().name() == "source")
        .and_then(|n| n.attribute("dir"))
        .map(str::to_string);

    let task = root
        .children()
        .filter(|n| n.is_element())
        .find(|n| n.tag_name().name() == "task");

    let (input, output, checker, subtasks) = match task {
        None => (None, None, None, Vec::new()),
        Some(task) => {
            let input = task.attribute("input").map(str::to_string);
            let output = task.attribute("output").map(str::to_string);
            let checker = task
                .attribute("checker")
                .map(strip_extension)
                .filter(|s| !s.is_empty());
            let mut subtasks = Vec::new();
            for sub in task.children().filter(|n| n.is_element()) {
                if sub.tag_name().name() != "subtask" {
                    continue;
                }
                let score = sub
                    .attribute("score")
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0);
                let mut points = Vec::new();
                for point in sub.children().filter(|n| n.is_element()) {
                    if point.tag_name().name() != "point" {
                        continue;
                    }
                    let in_file = point.attribute("in").unwrap_or_default().to_string();
                    let out_file = point.attribute("out").unwrap_or_default().to_string();
                    let time = point.attribute("time").and_then(|s| s.parse().ok());
                    let mem = point.attribute("mem").and_then(|s| s.parse().ok());
                    let submit = point.attribute("submit").map(str::to_string);
                    points.push(PointCfg {
                        in_file,
                        out_file,
                        time,
                        mem,
                        submit,
                    });
                }
                subtasks.push((score, points));
            }
            (input, output, checker, subtasks)
        }
    };

    Ok(PrbConfig {
        kind,
        source_dir,
        input,
        output,
        checker,
        subtasks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// 把 XML 写入临时 .prb 文件再解析。
    fn parse_str(xml: &str) -> Result<PrbConfig, String> {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.prb");
        std::fs::write(&path, xml).unwrap();
        parse_prb(&path)
    }

    /// 夹具 A：传统题，10 点均匀，fulltext。
    const PRB_A: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<problem type="TRA_0_4" maker="ccr-plus" version="1.1.0">
    <source dir="A" file="A" code="100">
        <language cmd="g++ -o %2 %1.cpp -lm -static" file="%1.cpp" time="10"/>
    </source>
    <task input="A.in" output="A.out" checker="fulltext" time="10">
        <subtask score="10">
            <point in="A1.in" out="A1.out" time="1" mem="128"/>
            <point in="A2.in" out="A2.out" time="1" mem="128"/>
        </subtask>
    </task>
</problem>
"#;

    /// 夹具 B：混合扩展名，每点时限/内存不同。
    const PRB_B: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<problem type="TRA_0_4" maker="ccr-plus" version="1.1.0">
    <source dir="B" file="B" code="100"/>
    <task input="B.in" output="B.out" checker="fulltext" time="10">
        <subtask score="20">
            <point in="B1.inp" out="B1.ans" time="0.5" mem="128"/>
            <point in="B4.inp" out="B4.ans" time="2" mem="512"/>
        </subtask>
    </task>
</problem>
"#;

    /// 夹具 C：自定义校验器 + 非均匀 subtask 30/70。
    const PRB_C: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<problem type="TRA_0_4" maker="ccr-plus" version="1.1.0">
    <source dir="C" file="C" code="100"/>
    <task input="C.in" output="C.out" checker="checker.cpp" time="10">
        <subtask score="30">
            <point in="C1.in" out="C1.out" time="1" mem="128"/>
        </subtask>
        <subtask score="70">
            <point in="C2.in" out="C2.out" time="1" mem="128"/>
        </subtask>
    </task>
</problem>
"#;

    /// 夹具 D：提交答案题。
    const PRB_D: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<problem type="ANS_0_4" maker="ccr-plus" version="1.1.0">
    <source dir="D" file="D" code="100"/>
    <task input="D.in" output="D.out" checker="fulltext" time="10">
        <subtask score="100">
            <point in="D1.in" out="D1.out" time="1" mem="128" submit="D1.out"/>
        </subtask>
    </task>
</problem>
"#;

    #[test]
    fn parses_traditional_config() {
        let cfg = parse_str(PRB_A).unwrap();
        assert_eq!(cfg.kind, ProblemKind::Traditional);
        assert_eq!(cfg.source_dir.as_deref(), Some("A"));
        assert_eq!(cfg.input.as_deref(), Some("A.in"));
        assert_eq!(cfg.output.as_deref(), Some("A.out"));
        assert_eq!(cfg.checker.as_deref(), Some("fulltext"));
        assert_eq!(cfg.subtasks.len(), 1);
        let (score, points) = &cfg.subtasks[0];
        assert_eq!(*score, 10);
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].in_file, "A1.in");
        assert_eq!(points[0].out_file, "A1.out");
        assert_eq!(points[0].time, Some(1.0));
        assert_eq!(points[0].mem, Some(128.0));
    }

    #[test]
    fn parses_per_point_limits() {
        let cfg = parse_str(PRB_B).unwrap();
        let (_, points) = &cfg.subtasks[0];
        assert_eq!(points[0].time, Some(0.5));
        assert_eq!(points[1].time, Some(2.0));
        assert_eq!(points[1].mem, Some(512.0));
    }

    #[test]
    fn parses_checker_extension_stripped() {
        let cfg = parse_str(PRB_C).unwrap();
        assert_eq!(cfg.checker.as_deref(), Some("checker"));
        let scores: Vec<u32> = cfg.subtasks.iter().map(|(s, _)| *s).collect();
        assert_eq!(scores, vec![30, 70]);
        assert_eq!(cfg.subtasks[1].1.len(), 1);
    }

    #[test]
    fn parses_answers_only_kind() {
        let cfg = parse_str(PRB_D).unwrap();
        assert_eq!(cfg.kind, ProblemKind::AnswersOnly);
        let (_, points) = &cfg.subtasks[0];
        assert_eq!(points[0].submit.as_deref(), Some("D1.out"));
    }

    #[test]
    fn parses_interactive_kind() {
        let cfg = parse_str(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<problem type="INT_0_4"><task/></problem>
"#,
        )
        .unwrap();
        assert_eq!(cfg.kind, ProblemKind::Interactive);
    }

    #[test]
    fn missing_type_defaults_to_unknown() {
        let cfg = parse_str(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<problem maker="ccr-plus"><task/></problem>
"#,
        )
        .unwrap();
        assert_eq!(cfg.kind, ProblemKind::Unknown);
    }

    #[test]
    fn missing_task_attributes_default_to_none() {
        let cfg = parse_str(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<problem type="TRA_0_4"><task/></problem>
"#,
        )
        .unwrap();
        assert_eq!(cfg.kind, ProblemKind::Traditional);
        assert_eq!(cfg.input, None);
        assert_eq!(cfg.output, None);
        assert_eq!(cfg.checker, None);
        assert!(cfg.subtasks.is_empty());
    }

    #[test]
    fn corrupted_xml_returns_err_not_panic() {
        let r = parse_str("<problem><task></problem");
        assert!(r.is_err());
    }

    #[test]
    fn non_problem_root_returns_err() {
        let r = parse_str(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<contest><order/></contest>
"#,
        );
        assert!(r.is_err());
    }

    #[test]
    fn missing_file_returns_err() {
        let tmp = TempDir::new().unwrap();
        assert!(parse_prb(&tmp.path().join("nope.prb")).is_err());
    }

    #[test]
    fn strip_extension_matches_ccr() {
        assert_eq!(strip_extension("fulltext"), "fulltext");
        assert_eq!(strip_extension("checker.cpp"), "checker");
        assert_eq!(strip_extension("a.b.c"), "a.b");
    }
}
