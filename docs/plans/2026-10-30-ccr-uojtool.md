# CCR-UOJtool 实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 桌面应用（Tauri 2）：打开一场 CCR-Plus 风格的比赛（通常 4 道题），提取题目并自动转换为 UOJ 风格 / Hydro 风格的题目数据包，逐题导出为 zip。

**Architecture:** Rust 转换引擎（纯库，不依赖 Tauri，可独立单测）负责解析 CCR-Plus 比赛目录、发现测试点、生成目标格式 zip；Tauri 2 IPC 命令把引擎暴露给 Web 前端（Vite + TypeScript）；外壳打包为桌面应用。

**Tech Stack:** Rust 1.98 + tauri 2 + roxmltree（解析 .prb/.ccr XML）+ zip（打包导出）+ serde；前端 Vite + TypeScript (vanilla) + @tauri-apps/api + plugin-dialog/fs。

---

## 0. 已查证的事实（执行者必读）

以下内容均已在设计阶段查证（2026-10-30），不要凭记忆修改。

### 0.1 CCR-Plus 比赛目录结构

来源：`sxyzccr/CCR-Plus` 源码 `src/common/contest.cpp` / `src/common/problem.cpp`。

```
<比赛目录>/
├── .ccr                    # XML，题目顺序（可能不存在）
├── data/                   # 数据目录（Global::data_path = path + "/data/"）
│   └── <题名>/             # 每道题一个目录；ReadFolders() 只把含 .prb 的目录当题目
│       ├── .prb            # XML 题目配置（未配置过的比赛可能没有）
│       ├── xxx1.in/.out    # 测试点数据
│       └── <checker>       # 可选：自定义校验器（编译产物或源码）
├── src/<选手>/<题目录>/     # 选手代码（本工具忽略）
└── result/                 # 评测结果（本工具忽略）
```

### 0.2 .prb XML 格式（CCR-Plus `SaveConfiguration()` 生成）

```xml
<?xml version="1.0" encoding="UTF-8"?>
<problem type="TRA_0_4" maker="ccr-plus" version="1.x.x">
    <source dir="A" file="A" code="100">
        <language cmd="g++ -o %2 %1.cpp -lm -static" file="%1.cpp" time="10"/>
        <language cmd="gcc -o %2 %1.c -lm -static" file="%1.c" time="10"/>
        <language cmd="fpc %1.pas" file="%1.pas" time="10"/>
    </source>
    <task input="A.in" output="A.out" checker="fulltext" time="10">
        <subtask score="10">
            <point in="A1.in" out="A1.out" time="1" mem="128"/>
            ...
        </subtask>
        ...
    </task>
</problem>
```

字段语义（对照 `Problem::ReadConfiguration()`）：
- 根节点 `type`：`TRA`/`TRA_0_4`=传统题，`ANS`/`ANS_0_4`=提交答案题，`INT`/`INT_0_4`=交互题；缺失时默认 `Global::OtherProblemType`。
- `<source dir>`：选手源码子目录名；`file`：可执行文件名；`code`：代码长度限制（KB）。
- `<task input/output>`：输入输出文件名模式；`checker`：内置校验器名（去掉扩展名后匹配 `BUILTIN_CHECKER_MAP`，目前内置只有 `fulltext`）；`time`：校验器时限。
- `<subtask score>`：子任务分值；`<point time mem>`：**每个测试点独立**时限（秒，浮点）/ 内存（MB，浮点）。`in`/`out` 是相对 `data/<题名>/` 的文件名。

### 0.3 测试点文件发现规则（CCR-Plus `getInAndOutFile()`）

- 输入扩展名候选：`.in` `.inp` `in` `inp`；输出扩展名候选：`.out` `.ans` `.ou` `.an` `.sol` `.res` `.std` `out` `ans` `ou` `an` `sol` `res` `std`。
- 配对逻辑：文件名中查找扩展名出现位置，去掉扩展名后的"前缀+后缀"相同的 in/out 文件配成一对（计数恰为 2）。
- 排序：前缀按**数字感知**排序（QCollator numeric mode，如 `A2 < A10`）。
- 没有 `.prb` 的题目目录不会被 CCR 识别为题目——本工具对缺 `.prb` 的目录做降级处理（见 0.6 决策 D6）。

### 0.4 UOJ 数据包格式（经典 UOJ / UniversalOJ）

来源：universaloj.github.io《传统题配置》、Rye_Catcher《配置UOJ数据的正确姿势》。

- **扁平 zip，不要子目录**。
- `problem.conf`（样例）：

  ```
  use_builtin_judger on
  use_builtin_checker ncmp
  n_tests 10
  n_ex_tests 1
  n_sample_tests 1
  input_pre www
  input_suf in
  output_pre www
  output_suf out
  time_limit 1
  memory_limit 512
  output_limit 64
  ```

- 标准测试点：`<input_pre>1.in`、`<input_pre>1.out`、… 编号从 1 开始。
- 额外测试点 `ex_` 前缀（样例放这里）——**本工具按决策 D4 不生成**（n_ex_tests=0, n_sample_tests=0）。
- `time_limit` 单位秒、经典文档要求整数（0.5s 需上取整并警告）；`memory_limit`/`output_limit` 单位 MB。
- checker 内建：`ncmp`(整数序列) `wcmp`(token 序列) `lcmp`(逐行 token 比较) `rcmp*`(浮点) 等。
- ⚠ 假设（未逐行核对 testlib 源码，已选择保守映射并允许用户覆盖）：CCR `fulltext`（过滤行末空格及文末回车）≈ UOJ `lcmp`。用户可在 UI 改成 `wcmp`/`ncmp`/`rcmp4` 等。

### 0.5 Hydro 题目包格式

来源：hydro.js.org《Hydro Problem Format》《测试数据格式》、DeepWiki（hydrojudge/src/cases.ts）。

```
<任意文件夹名>/
├── problem.yaml           # title / tag / pid
├── problem_zh.md          # Markdown 题面（本工具生成占位模板）
└── testdata/
    ├── config.yaml        # 可选；不提供时按文件名自动识别
    ├── 1.in / 1.out       # 文件名必须含数字
    └── ...
```

- `testdata/config.yaml` 关键字段：`type: default|interactive|submit_answer|objective`；`time: 1s`（字符串带单位，或毫秒整数）；`memory: 128m`（或 MB 整数）；`filename: foo`（文件 IO 前缀，标准 IO 则删掉）；`checker_type: default|ccr|cena|lemon|qduoj|syzoj|testlib...`；`checker: chk.cc`；`cases:`/`subtasks:` 显式列出测试点（每项 `{input, output, time?, memory?, score?}`）。
- **Hydro 内置 `checker_type: ccr`**——CCR 自定义校验器在 Hydro 侧可直接复用（V1 仅当 data 目录中存在校验器源文件时映射，否则警告）。
- ⚠ 假设（任务 6 中已安排验证步骤）：config.yaml 的 `time` 同时接受 `1s` 字符串与毫秒整数。为稳妥，生成时用**毫秒整数**。

### 0.6 已确认的需求决策

| # | 决策 |
|---|------|
| D1 | 架构：Web 技术前端 + Rust 后端引擎，打包为桌面应用 → **Tauri 2**（用户明确指定，拒绝 Python/PySide6/Electron） |
| D2 | 目标格式：**UOJ 经典 zip 与 Hydro 包都要** |
| D3 | 每测试点限制不一致 → UOJ/Hydro 统一限制取**最大值**，并在报告中警告 |
| D4 | 样例：**不生成** `ex_` 额外测试点（n_ex_tests=0）；题面样例由人工在 OJ 后台处理 |
| D5 | checker 映射：`fulltext` → UOJ `lcmp` / Hydro `default`；自定义校验器 → Hydro 尽量映射（`checker_type: ccr`）、UOJ 警告需人工适配；UI 可覆盖 |
| D6 | 无 `.prb` 的题目目录：按传统题降级解析（默认 fulltext、时限/内存取 UI 默认值），报告提示"该目录未在 CCR 中配置" |
| D7 | 提交答案题(ANS)/交互题(INT)：V1 **跳过 + 警告**，不转换 |
| D8 | CCR subtask 非均匀分值：UOJ 传统题只能等分（100/N），保留警告；V1 不做 Hydro subtask 映射（记入未来工作） |
| D9 | 题数不写死 4：引擎处理 N 题，"四道"是典型场景 |
| D10 | 题面（statement）：CCR 比赛数据不含题面，V1 只生成占位模板（Hydro）或提示手动录入（UOJ 网页端） |

### 0.7 格式映射总表（核心逻辑，引擎按此实现）

| CCR-Plus | → UOJ | → Hydro |
|---|---|---|
| `data/<题>/` + `.prb` | 扁平 zip（无子目录） | zip 内 `<题名>/` 文件夹 |
| `<point>` 列表（数字感知排序） | `<pre>1.in/.out` … `<pre>N.in/.out` | `testdata/1.in/.out` … |
| 每点 time(s)/mem(MB) 取最大 | `time_limit` = ceil(max_time)，`memory_limit` = ceil(max_mem) | `time: <max*1000>`(ms 整数)，`memory: <max_mem>` |
| `<task input="A.in">` | `input_pre` = 题名（UI 可改），`input_suf in` | `filename: A`（去掉 `.in`） |
| `checker=fulltext` | `use_builtin_checker lcmp` | `checker_type: default` |
| 自定义校验器 | 警告：接口不兼容，需人工改写 testlib checker | 有源码则复制 + `checker_type: ccr`；否则警告 |
| subtask 分值 | 等分 100/N（非均匀时警告） | `cases` 平铺（V1 不映射 subtask） |
| ANS/INT 题 | 跳过 + 警告 | 跳过 + 警告 |
| `.prb` 不存在 | 降级为传统题 + 警告 | 同左 |

---

## 1. 项目结构（最终形态）

```
CCR-UOJtool/
├── .gitignore
├── package.json
├── tsconfig.json
├── vite.config.ts
├── index.html
├── src/                          # 前端
│   ├── main.ts
│   ├── style.css
│   └── types.ts                  # 与 Rust DTO 对应的 TS 类型
├── src-tauri/
│   ├── Cargo.toml
│   ├── build.rs
│   ├── tauri.conf.json
│   ├── capabilities/default.json
│   ├── icons/                    # 脚手架生成
│   └── src/
│       ├── main.rs               # tauri::Builder 入口
│       ├── lib.rs                # run() + 插件注册 + invoke_handler
│       ├── commands.rs           # scan_contest / import_contest_zip / export_problems
│       └── engine/               # 纯 Rust 引擎（不依赖 tauri）
│           ├── mod.rs
│           ├── model.rs          # Contest / Problem / TestCase / DTO
│           ├── discover.rs       # 目录扫描 + in/out 配对 + 数字感知排序
│           ├── prb.rs            # .prb XML 解析（roxmltree）
│           ├── ccr.rs            # .ccr 顺序 + 比赛组装（含降级逻辑）
│           ├── report.rs         # Warning / ExportReport
│           ├── uoj.rs            # UOJ problem.conf + zip
│           ├── hydro.rs          # Hydro problem.yaml + config.yaml + zip
│           └── ziputil.rs        # 共用 zip 写入助手
└── docs/plans/2026-10-30-ccr-uojtool.md
```

Rust 关键依赖（`src-tauri/Cargo.toml`）：

```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-dialog = "2"
tauri-plugin-fs = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
roxmltree = "0.20"
zip = { version = "2", default-features = false, features = ["deflate"] }
thiserror = "2"
tempfile = "3"          # dev 用途也放在 dependencies 之外；见 Task 3
```

---

## 2. 任务清单

约定：所有 `cargo test` 在 `src-tauri/` 下运行；所有 npm 命令在仓库根目录运行。每个任务结束必须 commit。TDD 顺序不可调换：先写失败测试再实现。

### Phase 0 脚手架

### Task 1: 环境准备与 Tauri 脚手架

**Files:** 仓库根目录全部脚手架文件。

1. 检查 Linux 系统依赖（Tauri v2 需要 webkit2gtk-4.1）：
   ```bash
   pkg-config --exists webkit2gtk-4.1 && echo OK || echo MISSING
   ```
   若 MISSING：`sudo apt install -y libwebkit2gtk-4.1-dev build-essential libssl-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev`（需用户确认后执行）。
2. 在临时目录生成脚手架再并入仓库（目录已有 .git，create-tauri-app 不支持原地初始化）：
   ```bash
   cd /tmp && npm create tauri-app@latest ccr-scaffold -- \
     --template vanilla-ts --manager npm --identifier com.quzhou2.ccr2uoj --yes
   rsync -a --exclude .git /tmp/ccr-scaffold/ /home/qzez/CCR-UOJtool/
   cd /home/qzez/CCR-UOJtool && npm install
   ```
3. 确认 `package.json` 有 `"tauri": "tauri"` script；`src-tauri/Cargo.toml` 的 `[profile]` 保留默认。`git branch -m main`（与 GitHub 默认分支一致）。
4. 验证（无显示器环境验证编译，不开窗口）：
   ```bash
   npx tauri info          # 确认环境识别正常
   cd src-tauri && cargo check
   ```
   Expected: 编译通过，无 error。
5. Commit: `chore: scaffold tauri2 + vanilla-ts`

### Task 2: 添加引擎依赖与 .gitignore

**Files:** Modify `src-tauri/Cargo.toml`、`.gitignore`。

1. `.gitignore` 追加：`node_modules/`、`dist/`、`src-tauri/target/`。
2. `cd src-tauri && cargo add roxmltree@0.20 zip@2 --no-default-features --features zip/deflate serde@1 --features serde/derive serde_json@1 thiserror@2 && cargo add tempfile@3 --dev`
3. 验证：`cargo check` 通过。
4. Commit: `chore: add engine dependencies`

### Phase 1 CCR 解析引擎

### Task 3: 数据模型与测试夹具助手

**Files:**
- Create: `src-tauri/src/engine/mod.rs`、`src-tauri/src/engine/model.rs`
- Create: `src-tauri/tests/common/mod.rs`（夹具构造助手）
- Modify: `src-tauri/src/lib.rs`（`mod engine;`）

1. 写失败测试 `src-tauri/src/engine/model.rs` 内联测试 + `tests/common/mod.rs` 提供函数 `build_fixture_contest(root: &Path)`，构造如下夹具（覆盖 4 道题的典型/边界场景）：

   ```
   root/
   ├── .ccr                         # order: A B C D
   ├── data/A/                      # 传统题，均匀 10 点，1s/128MB，fulltext
   │   ├── .prb
   │   └── A1.in/A1.out ... A10.in/A10.out
   ├── data/B/                      # 混合扩展名 .inp/.ans，每点时限不同(0.5~2s)
   │   ├── .prb
   │   └── B1.inp/B1.ans ... B5.inp/B5.ans
   ├── data/C/                      # 自定义校验器 checker.cpp，subtask 30/70 非均匀
   │   ├── .prb / checker.cpp
   │   └── C1.in/.out C2.in/.out
   └── data/D/                      # 提交答案题 ANS + 乱序文件名 D10,D2,...
       ├── .prb (type="ANS_0_4")
       └── D2.in/D10.out D10.in/D2.out ...
   ```

2. 在 `model.rs` 定义核心类型（完整代码）：

   ```rust
   use serde::{Deserialize, Serialize};
   use std::path::{Path, PathBuf};

   #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
   pub enum ProblemKind { Traditional, AnswersOnly, Interactive, Unknown }

   #[derive(Debug, Clone, Serialize)]
   pub struct TestCase {
       pub in_file: PathBuf,      // 相对 data/<题名>/
       pub out_file: PathBuf,
       pub time_limit: f64,       // 秒
       pub memory_limit: f64,     // MB
       pub score: u32,
   }

   #[derive(Debug, Clone, Serialize)]
   pub struct Problem {
       pub name: String,          // 目录名
       pub kind: ProblemKind,
       pub in_file: Option<String>,   // <task input>，如 "A.in"
       pub out_file: Option<String>,
       pub checker: Option<String>,   // 去扩展名后，如 "fulltext"
       pub custom_checker: Option<String>, // data 目录中的校验器文件名（若有）
       pub cases: Vec<TestCase>,      // 数字感知排序后
       pub warnings: Vec<String>,     // 解析阶段警告
       pub prb_found: bool,
   }

   impl Problem {
       /// 统一限制取最大值（决策 D3）
       pub fn max_time(&self) -> f64 { self.cases.iter().map(|c| c.time_limit).fold(f64::MIN, f64::max) }
       pub fn max_mem(&self) -> f64 { self.cases.iter().map(|c| c.memory_limit).fold(f64::MIN, f64::max) }
       /// subtask 分值是否均匀等分
       pub fn uniform_score(&self) -> bool { /* 所有 case score 相等即视为均匀 */ }
   }

   #[derive(Debug, Serialize)]
   pub struct Contest {
       pub name: String,
       pub path: PathBuf,
       pub problems: Vec<Problem>,
   }
   ```

3. 实现使测试通过的最小代码（uniform_score 等纯函数带单元测试）。
4. `cargo test -p ccr-uojtool --lib`（crate 名以脚手架实际为准，下同）Expected: PASS。
5. Commit: `feat(engine): data model + fixture helper`

### Task 4: 测试点发现（in/out 配对 + 数字感知排序）

**Files:**
- Create: `src-tauri/src/engine/discover.rs`
- Test: `src-tauri/src/engine/discover.rs` 内联 `#[cfg(test)]`

1. 失败测试用例（至少 6 个）：
   - `A1.in + A1.out` → 配对，key=`A1`
   - 混合扩展名 `B1.inp + B1.ans` → 配对
   - `C10.in` 排在 `C2.in` 之后（数字感知）
   - 只有 `.in` 没有 `.out` → 忽略并产生警告
   - 目录里混入无关文件（`std.cpp`、`.prb`、`checker.cpp`）→ 不配对
   - 无扩展名形式 `2 in / 2 out`（CCR 支持空格前缀形态，如 `2 in`，即文件名 `2 in`/`2 out`）→ 配对
2. 实现（对齐 0.3 规则）：

   ```rust
   pub const IN_EXTS: &[&str]  = &[".in", ".inp", "in", "inp"];
   pub const OUT_EXTS: &[&str] = &[".out", ".ans", ".ou", ".an", ".sol", ".res", ".std",
                                    "out", "ans", "ou", "an", "sol", "res", "std"];

   /// 在文件名 name 中找到扩展名 ext 的出现位置 p，返回 (前缀, 后缀)。
   /// in/out 两表的 (前缀,后缀) 完全一致时配对（对应 CCR getInAndOutFile 的计数==2 逻辑）。
   pub fn discover_cases(dir: &Path) -> (Vec<(PathBuf, PathBuf)>, Vec<String> /*warnings*/);

   /// 数字感知比较：按 digit 段拆分，数字段按数值比较，其余按字节比较。
   pub fn natural_cmp(a: &str, b: &str) -> Ordering;
   ```

   实现要点：对目录内每个普通文件，对每个 (in_ext, out_ext) 组合扫描；注意无点扩展名（`in`/`inp`）按"查找子串"处理以兼容 CCR 语义；统计 key→(in,out) 计数恰为 2 才收 pair。实现后用 `discover_cases` 结果 + `natural_cmp` 排序。
3. `cargo test --lib discover` Expected: PASS。
4. Commit: `feat(engine): testcase discovery with natural sort`

### Task 5: .prb XML 解析

**Files:**
- Create: `src-tauri/src/engine/prb.rs`
- Test: 内联测试（用 Task 3 夹具中的 A/B/C/D 四份 .prb 字符串直接内嵌测试）

1. 失败测试：
   - 解析 TRA_0_4：kind、checker、cases 数量、每点 time/mem、subtask 分值
   - 解析 ANS_0_4：kind=AnswersOnly
   - 损坏 XML / 根节点非 `<problem>`：返回带警告的空配置（不 panic）
   - `<task input/output/checker>` 缺省时的字段缺省
2. 实现（roxmltree，完整逻辑）：

   ```rust
   pub struct PrbConfig {
       pub kind: ProblemKind,
       pub source_dir: Option<String>,
       pub input: Option<String>,
       pub output: Option<String>,
       pub checker: Option<String>,
       pub subtasks: Vec<(u32, Vec<PointCfg>)>,   // (score, points)
   }
   pub struct PointCfg { pub in_file: String, pub out_file: String,
                         pub time: Option<f64>, pub mem: Option<f64>, pub submit: Option<String> }

   pub fn parse_prb(path: &Path) -> Result<PrbConfig, String>;
   ```

   根节点 `type` 属性按 0.2 的映射表转换；`checker` 属性去扩展名（与 CCR `RemoveFileExtension` 一致，只去最后一个 `.` 之后）。
3. `cargo test --lib prb` Expected: PASS。
4. Commit: `feat(engine): parse .prb xml`

### Task 6: 比赛组装（.ccr 顺序 + 目录扫描 + 降级）

**Files:**
- Create: `src-tauri/src/engine/ccr.rs`
- Test: `src-tauri/tests/contest_discovery.rs`（集成测试，用 Task 3 夹具）

1. 失败测试（针对夹具）：
   - 发现 A/B/C/D 四题，顺序与 `.ccr` 一致（A,B,C,D）
   - A：10 点，uniform，1s/128MB，fulltext，prb_found=true
   - B：5 点，混合扩展名配对成功，max_time=2.0
   - C：custom_checker=Some("checker.cpp")，非均匀 subtask 警告存在
   - D：kind=AnswersOnly
   - 删除 C 的 .prb 后重扫：C 降级为传统题、prb_found=false、带"未配置"警告，点数按 discover 推断，限制用传入默认值（1.0s/128MB）
2. 实现：

   ```rust
   pub struct ScanOptions { pub default_time: f64, pub default_mem: f64 }  // 默认 1.0 / 128.0
   pub fn scan_contest(path: &Path, opts: &ScanOptions) -> Result<Contest, String>;
   ```
   逻辑：读 `.ccr`（XML `<contest><order><problem>名</problem>`）得顺序 → 扫 `data/` 下含 `.prb` 或含配对数据的目录 → 无 `.prb` 的目录降级（D6）→ 有 `.prb` 的解析并用其 point 列表与 discover 结果**对账**（.prb 里的 point 文件必须存在；discover 多出的文件产生警告）→ 按 `.ccr` 顺序输出，未列入 order 的题排后面（字母序）。
3. `cargo test --test contest_discovery` Expected: PASS。
4. Commit: `feat(engine): contest assembly with fallback`

### Phase 2 UOJ 导出

### Task 7: UOJ problem.conf 生成 + checker 映射

**Files:**
- Create: `src-tauri/src/engine/uoj.rs`
- Test: 内联

1. 失败测试：
   - A 题 → conf 文本精确匹配（完整断言）：

     ```
     use_builtin_judger on
     use_builtin_checker lcmp
     n_tests 10
     n_ex_tests 0
     n_sample_tests 0
     input_pre A
     input_suf in
     output_pre A
     output_suf out
     time_limit 1
     memory_limit 128
     output_limit 64
     ```

   - B 题 max_time=2.0 → `time_limit 2`；构造 0.5s 案例 → `time_limit 1`（ceil）且警告"时限上取整"
   - `fulltext`→`lcmp`；`nonte xt`（用户覆盖）→ 原样输出；max_mem 128.0 → `memory_limit 128`
2. 实现：

   ```rust
   pub struct UojOptions { pub checker_override: Option<String> }
   pub fn build_problem_conf(p: &Problem, opts: &UojOptions) -> (String, Vec<String> /*warnings*/);
   pub fn map_checker(ccr_checker: &str) -> &'static str;   // "fulltext" => "lcmp"，其他 => "lcmp" + 警告
   ```
3. `cargo test --lib uoj::conf` Expected: PASS。
4. Commit: `feat(uoj): problem.conf generation`

### Task 8: UOJ zip 打包

**Files:**
- Create: `src-tauri/src/engine/ziputil.rs`、`uoj.rs` 追加
- Test: 内联（导出到 tempfile 临时目录，用 `zip::ZipArchive` 读回断言）

1. 失败测试：A 题导出 → zip 内**根层级**恰有 `problem.conf` + `A1.in..A10.in` + `A1.out..A10.out` 共 21 项（无子目录，验证 0.4"不要建子目录"）；conf 内容与 Task 7 一致；数据字节与源文件一致。
2. 实现：

   ```rust
   pub fn export_problem_uoj(p: &Problem, contest_path: &Path, out_dir: &Path,
                             opts: &UojOptions) -> Result<UojExportItem, EngineError>;
   // 产出 out_dir/<题名>.zip；文件名 <input_pre><i>.in/.out，i 从 1
   ```
   `ziputil.rs` 提供 `fn write_entry<W: Write + Seek>(zip: &mut ZipWriter<W>, name: &str, bytes: &[u8])` 与流式文件拷贝助手（数据文件可能大，勿整体读内存）。
   跳过规则（D7）：kind != Traditional → 返回 skipped + 警告，不产 zip。
3. `cargo test --lib uoj::zip` Expected: PASS。
4. Commit: `feat(uoj): flat zip export`

### Phase 3 Hydro 导出

### Task 9: Hydro problem.yaml / config.yaml / 题面模板生成

**Files:**
- Create: `src-tauri/src/engine/hydro.rs`
- Test: 内联

1. 失败测试：
   - A 题 config.yaml 精确匹配：

     ```yaml
     type: default
     time: 1000
     memory: 128
     filename: A
     checker_type: default
     cases:
       - input: 1.in
         output: 1.out
       - input: 2.in
         output: 2.out
       ...(共 10 项)
     ```

   - problem.yaml：`title: A`；题面模板 `problem_zh.md` 含 `# A` 占位标题与"题面待补充"提示
   - B 题 max_time=2.0 → `time: 2000`（ms 整数，见 0.5 假设；实现处注释标注）
   - C 题存在 checker.cpp → `checker_type: ccr` + `checker: checker.cpp`；否则 `default`
2. 实现（字符串模板手工拼接，不引入 serde_yaml，保持确定性输出顺序）：

   ```rust
   pub fn build_config_yaml(p: &Problem, opts: &HydroOptions) -> (String, Vec<String>);
   pub fn build_problem_yaml(p: &Problem) -> String;
   pub fn build_statement_md(p: &Problem) -> String;
   ```
3. `cargo test --lib hydro::yaml` Expected: PASS。
4. Commit: `feat(hydro): yaml generation`

### Task 10: Hydro zip 打包

**Files:** Modify `hydro.rs`、Test 内联

1. 失败测试：A 题导出 → zip 内路径为 `<题名>/problem.yaml`、`<题名>/problem_zh.md`、`<题名>/testdata/config.yaml`、`<题名>/testdata/1.in` …（对齐 0.5 结构，保留一层文件夹）；数据字节一致；D 题（ANS）跳过并警告。
2. 实现 `export_problem_hydro(...)`（同 Task 8 签名风格，复用 ziputil）。
3. `cargo test --lib hydro::zip` Expected: PASS。
4. Commit: `feat(hydro): zip export`

### Phase 4 编排与报告

### Task 11: 导出编排 + 端到端测试

**Files:**
- Create: `src-tauri/src/engine/report.rs`、`mod.rs` 追加编排函数
- Test: `src-tauri/tests/export_e2e.rs`

1. report.rs 类型（serde Serialize，供前端直接消费）：

   ```rust
   #[derive(Serialize)] pub struct ProblemReport {
       pub problem: String, pub target: String, pub zip_path: Option<String>,
       pub n_tests: usize, pub skipped: bool,
       pub warnings: Vec<String>, pub errors: Vec<String>,
   }
   #[derive(Serialize)] pub struct ExportReport { pub items: Vec<ProblemReport> }
   ```

2. 编排函数：

   ```rust
   pub enum Target { Uoj, Hydro, Both }
   pub fn export_contest(contest: &Contest, target: Target, out_dir: &Path,
                          opts: &ExportOptions) -> Result<ExportReport, EngineError>;
   ```
   `ExportOptions` 承载每题的用户编辑（target_id 覆盖、time/mem 覆盖、checker 覆盖、enabled）。
3. 失败测试（端到端，对 Task 3 夹具）：`export_contest(Both, tmp)` → 输出目录含 `A.zip B.zip C.zip`（UOJ 命名 `<题名>.zip`）与 Hydro 版 `A.zip B.zip C.zip`（输出到 `out/uoj/`、`out/hydro/` 子目录避免重名）；D 题出现在报告中且 `skipped=true`；C 题警告含"非均匀子任务分值"与"自定义校验器"。逐 zip 读回验证条目数与 conf/yaml 内容。
4. 实现 → `cargo test --test export_e2e` Expected: PASS。
5. Commit: `feat(engine): export orchestration + e2e`

### Phase 5 Tauri 集成

### Task 12: IPC 命令层

**Files:**
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

1. 三个命令（参数/返回值全部 serde，前端拿 JSON）：

   ```rust
   #[tauri::command]
   pub fn scan_contest(path: String) -> Result<ContestDto, String>;        // 引擎 Contest → DTO
   #[tauri::command]
   pub fn import_contest_zip(zip_path: String) -> Result<ContestDto, String>;
   // 解压到 std::env::temp_dir().join("ccr-uojtool/<zip文件名去扩展名>-<pid或时间戳>") 再 scan
   #[tauri::command]
   pub fn export_problems(contest_path: String, edits: Vec<ProblemEditDto>,
                          target: String /* "uoj"|"hydro"|"both" */, out_dir: String)
       -> Result<ExportReport, String>;
   ```
   `ContestDto`/`ProblemEditDto` 定义在 `model.rs`（`#[derive(Serialize, Deserialize)]`）。错误一律 `String`（Tauri 命令要求可序列化错误；EngineError 实现 Display）。
2. `lib.rs` 注册：`.plugin(tauri_plugin_dialog::init()).plugin(tauri_plugin_fs::init())` + `invoke_handler![scan_contest, import_contest_zip, export_problems]`。
3. 验证：`cargo check` 通过；`cargo test` 全绿（命令层薄封装，靠引擎测试覆盖）。
4. Commit: `feat(app): tauri ipc commands`

### Task 13: 前端 UI

**Files:**
- Modify: `index.html`、`src/main.ts`、`src/style.css`
- Create: `src/types.ts`

1. `types.ts`：与 `ContestDto`/`ProblemEditDto`/`ExportReport` 对应的 TS interface。
2. `index.html` 结构：

   ```
   <header> 标题 | [打开比赛目录] [导入比赛 zip] | 输出目录选择 | [导出 UOJ] [导出 Hydro] [导出两者] </header>
   <main>
     <aside id="problem-list">        <!-- 题目卡片：名称、类型徽章、点数、时限范围、checker -->
     <section id="problem-detail">    <!-- 选中题：可编辑 target_id / 时限 / 内存 / checker 覆盖、enabled 勾选
                                            + 只读测试点映射表：原文件 → 新文件名、每点 time/mem -->
   <dialog id="report-dialog">         <!-- 导出结果：逐题 zip 路径 + 警告/错误列表 -->
   ```

3. `main.ts` 要点（vanilla TS）：
   - 打开目录：`dialog.open({ directory: true })` → `invoke("scan_contest", { path })`
   - 导入 zip：`dialog.open({ filters: [{ name: "CCR 比赛", extensions: ["zip"] }] })` → `invoke("import_contest_zip", ...)`
   - 渲染题目列表与详情；详情中的输入框绑定 `edits[题名]`，默认值取引擎推断（target_id=题名、time=ceil(max_time)、mem=ceil(max_mem)、checker=映射结果）
   - 导出：`dialog.open({ directory: true })` 选输出目录 → `invoke("export_problems", { contestPath, edits, target, outDir })` → 渲染报告 dialog；提供"打开输出目录"按钮（`plugin-opener` 或 dialog 插件路径显示即可，V1 只显示路径）
   - ANS/INT 题在列表中置灰并标注"V1 不支持转换"
4. `style.css`：简洁两栏布局（左 280px 列表 + 右自适应），系统字体，警告用琥珀色、错误红色徽章。不做组件框架（YAGNI）。
5. 验证：`npm run build`（vite build + tsc）无类型错误；有显示环境则 `npm run tauri dev` 手动走通"打开→预览→导出"。
6. Commit: `feat(ui): contest browse, edit, export`

### Phase 6 打包与收尾

### Task 14: 打包验证

1. Linux: `npm run tauri build` → 确认产出 `src-tauri/target/release/bundle/deb/*.deb` 与 `appimage/*.AppImage`。
2. Windows 安装包：本机为 Linux 无法交叉编译。Create `.github/workflows/release.yml`（tauri-apps/tauri-action，matrix: windows-latest），打 tag `v*` 时构建并上传 artifact。（可选任务，需用户确认后创建。）
3. Commit: `build: packaging + release workflow`

### Task 15: README 与文档

**Files:** Create `README.md`

1. 内容：工具用途截图位、安装（下载 release / 从源码构建步骤）、使用流程（打开比赛→检查映射→导出→上传 OJ）、**格式映射总表（照抄 0.7）**、已知限制（ANS/INT 跳过、样例不生成、UOJ 自定义 checker 需人工适配、题面需手动录入）。
2. Commit: `docs: readme`
3. 推送前征求用户确认：`git push -u origin main`（首次推送、影响远端，必须用户确认）。

---

## 3. 验收清单

- [ ] 打开 CCR-Plus 比赛目录（含/不含 .prb）能正确列出题目（夹具四题全部识别，顺序正确）
- [ ] 每题测试点映射表正确（配对、数字感知排序、每点限制）
- [ ] UOJ 导出：4 题 → 4 个扁平 zip，problem.conf 字段完整正确，A 题可在真实 UOJ 上传成功
- [ ] Hydro 导出：zip 结构含一层题名文件夹，config.yaml 可被 Hydro 导入识别
- [ ] 限制不一致、非均匀分值、缺 .out、ANS/INT、自定义校验器等场景均有警告且不阻塞其他题
- [ ] `cargo test` 与 `npm run build` 全绿

## 4. 已知限制与未来工作

- 不映射 Hydro subtask 分值（D8）——数据在 `.prb` 中已有，未来可加 `subtasks:` 输出
- 不生成/推断 std.cpp、val.cpp（UOJ hack 功能需人工补充）
- 题面不转换（D10）；Hydro 仅占位模板
- UOJ 自定义校验器无法自动转换（接口不同，0.7 表）
- 提交答案题/交互题转换（UOJ submit_answer / interactive 配置）
