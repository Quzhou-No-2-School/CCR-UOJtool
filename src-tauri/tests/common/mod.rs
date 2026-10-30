//! 集成测试共享夹具：构造 CCR-Plus 风格比赛目录（计划 Task 3）。
//!
//! 四道题覆盖典型/边界场景：
//! - A：传统题，均匀 10 点（1s/128MB），fulltext
//! - B：混合扩展名 .inp/.ans，每点时限不同（0.5~2s）、内存不同
//! - C：自定义校验器 checker.cpp，subtask 30/70 非均匀
//! - D：提交答案题（ANS_0_4），文件按乱序创建（D10 先于 D2）

use std::fs;
use std::path::Path;

/// 在 `root` 下构造完整比赛夹具。
pub fn build_fixture_contest(root: &Path) {
    let data = root.join("data");
    fs::create_dir_all(&data).unwrap();

    fs::write(
        root.join(".ccr"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<contest>
    <order>
        <problem>A</problem>
        <problem>B</problem>
        <problem>C</problem>
        <problem>D</problem>
    </order>
</contest>
"#,
    )
    .unwrap();

    build_problem_a(&data);
    build_problem_b(&data);
    build_problem_c(&data);
    build_problem_d(&data);
}

fn write_pair(dir: &Path, in_name: &str, out_name: &str) {
    fs::write(dir.join(in_name), format!("{in_name}\n")).unwrap();
    fs::write(dir.join(out_name), format!("{out_name}\n")).unwrap();
}

/// A：传统题，单一 subtask(score=10) 含 10 点，每点 1s/128MB，fulltext。
fn build_problem_a(data: &Path) {
    let dir = data.join("A");
    fs::create_dir_all(&dir).unwrap();

    let mut points = String::new();
    for i in 1..=10 {
        write_pair(&dir, &format!("A{i}.in"), &format!("A{i}.out"));
        points.push_str(&format!(
            r#"            <point in="A{i}.in" out="A{i}.out" time="1" mem="128"/>"#,
        ));
        points.push('\n');
    }

    fs::write(
        dir.join(".prb"),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<problem type="TRA_0_4" maker="ccr-plus" version="1.1.0">
    <source dir="A" file="A" code="100">
        <language cmd="g++ -o %2 %1.cpp -lm -static" file="%1.cpp" time="10"/>
        <language cmd="gcc -o %2 %1.c -lm -static" file="%1.c" time="10"/>
        <language cmd="fpc %1.pas" file="%1.pas" time="10"/>
    </source>
    <task input="A.in" output="A.out" checker="fulltext" time="10">
        <subtask score="10">
{points}        </subtask>
    </task>
</problem>
"#
        ),
    )
    .unwrap();
}

/// B：混合扩展名 .inp/.ans，5 点时限 0.5/1/1.5/2/1 秒、内存 128/256/128/512/128。
fn build_problem_b(data: &Path) {
    let dir = data.join("B");
    fs::create_dir_all(&dir).unwrap();

    let times = ["0.5", "1", "1.5", "2", "1"];
    let mems = ["128", "256", "128", "512", "128"];
    let mut points = String::new();
    for i in 1..=5 {
        write_pair(&dir, &format!("B{i}.inp"), &format!("B{i}.ans"));
        points.push_str(&format!(
            r#"            <point in="B{i}.inp" out="B{i}.ans" time="{}" mem="{}"/>"#,
            times[i - 1], mems[i - 1],
        ));
        points.push('\n');
    }

    fs::write(
        dir.join(".prb"),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<problem type="TRA_0_4" maker="ccr-plus" version="1.1.0">
    <source dir="B" file="B" code="100">
        <language cmd="g++ -o %2 %1.cpp -lm -static" file="%1.cpp" time="10"/>
    </source>
    <task input="B.in" output="B.out" checker="fulltext" time="10">
        <subtask score="20">
{points}        </subtask>
    </task>
</problem>
"#
        ),
    )
    .unwrap();
}

/// C：自定义校验器 checker.cpp，两个 subtask 分值 30/70 非均匀，各 1 点。
fn build_problem_c(data: &Path) {
    let dir = data.join("C");
    fs::create_dir_all(&dir).unwrap();

    write_pair(&dir, "C1.in", "C1.out");
    write_pair(&dir, "C2.in", "C2.out");
    fs::write(dir.join("checker.cpp"), "// custom checker\n").unwrap();

    fs::write(
        dir.join(".prb"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<problem type="TRA_0_4" maker="ccr-plus" version="1.1.0">
    <source dir="C" file="C" code="100">
        <language cmd="g++ -o %2 %1.cpp -lm -static" file="%1.cpp" time="10"/>
    </source>
    <task input="C.in" output="C.out" checker="checker.cpp" time="10">
        <subtask score="30">
            <point in="C1.in" out="C1.out" time="1" mem="128"/>
        </subtask>
        <subtask score="70">
            <point in="C2.in" out="C2.out" time="1" mem="128"/>
        </subtask>
    </task>
</problem>
"#,
    )
    .unwrap();
}

/// D：提交答案题（ANS_0_4），D1..D10 按乱序创建以测试数字感知排序。
fn build_problem_d(data: &Path) {
    let dir = data.join("D");
    fs::create_dir_all(&dir).unwrap();

    // 乱序创建：10, 1, 9, 2, 8, 3, ...
    let order = [10usize, 1, 9, 2, 8, 3, 7, 4, 6, 5];
    let mut points = String::new();
    for &i in &order {
        write_pair(&dir, &format!("D{i}.in"), &format!("D{i}.out"));
    }
    for i in 1..=10 {
        points.push_str(&format!(
            r#"            <point in="D{i}.in" out="D{i}.out" time="1" mem="128"/>"#,
        ));
        points.push('\n');
    }

    fs::write(
        dir.join(".prb"),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<problem type="ANS_0_4" maker="ccr-plus" version="1.1.0">
    <source dir="D" file="D" code="100">
        <language cmd="g++ -o %2 %1.cpp -lm -static" file="%1.cpp" time="10"/>
    </source>
    <task input="D.in" output="D.out" checker="fulltext" time="10">
        <subtask score="100">
{points}        </subtask>
    </task>
</problem>
"#
        ),
    )
    .unwrap();
}
