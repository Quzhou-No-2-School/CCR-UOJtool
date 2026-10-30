# CCR-UOJtool

把 CCR-Plus 评测器的比赛数据，一键转换为 UOJ / Hydro 在线评测系统的题目数据包。

CCR-UOJtool 是一款桌面应用（Windows / Linux），面向学校信息学竞赛教练与比赛组织者：用 CCR-Plus 在本地评完一场比赛后（通常 4 道题），无需再手工整理测试点、时限和校验器配置——直接打开比赛目录，逐题检查映射，即可导出符合目标 OJ 格式的 zip 数据包，上传后即可开题。

## 功能特性

- **打开即用**：打开 CCR-Plus 比赛目录，或导入打包好的比赛 zip，自动识别其中的题目与测试点
- **智能配对**：支持 `.in/.out`、`.inp/.ans` 等多种扩展名配对，数字感知排序（`A2` 排在 `A10` 之前）
- **配置解析**：读取 CCR 的 `.prb` 题目配置，保留每测试点独立的时限/内存限制与子任务分值
- **自动降级**：未在 CCR 中配置过（缺少 `.prb`）的题目目录，自动按传统题处理并给出提示
- **双格式导出**：UOJ 经典格式（扁平 zip + `problem.conf`）与 Hydro 格式（`problem.yaml` + `testdata/config.yaml`），可单独或同时导出
- **导出前可调整**：逐题修改题号、时限、内存、校验器
- **导出报告**：逐题列出警告与错误，单题异常不影响其他题导出

## 下载安装

### 预编译版本

从 [Releases](../../releases) 页面下载对应平台的安装包：

- Linux：`.deb`（Debian/Ubuntu 系）或 `.AppImage`（通用）
- Windows：`.msi` 安装包（计划通过 CI 自动发布）

### 从源码构建

环境要求：

| 依赖 | 版本要求 |
|---|---|
| Rust | 1.98+（通过 [rustup](https://rustup.rs) 安装） |
| Node.js | 22+ |

Linux 还需安装系统依赖（Tauri 需要 webkit2gtk-4.1）：

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential libssl-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev
```

构建步骤：

```bash
npm install
npm run tauri build
```

构建产物位于 `src-tauri/target/release/bundle/` 下（`.deb`、`.AppImage` 等）。

## 使用流程

1. **打开比赛**：点击"打开比赛目录"选择 CCR-Plus 的比赛目录，或点击"导入比赛 zip"选择打包好的比赛 zip。
2. **检查每题映射**：左侧题目列表逐题查看题目类型、测试点数量、时限/内存范围与校验器；右侧可修改题号、统一时限/内存、校验器。有异常的题（如各测试点时限不一致、子任务分值不均匀）会显示琥珀色警告；提交答案题/交互题会置灰并标注"V1 不支持转换"。
3. **选择导出格式**：UOJ / Hydro / 两者。
4. **导出 zip**：选择输出目录，导出完成后在报告对话框中逐题确认 zip 路径与警告信息。
5. **上传 OJ**：登录 UOJ / Hydro 管理后台，逐题上传对应的 zip 数据包。

### CCR-Plus 比赛目录结构示意

CCR-UOJtool 读取的是 CCR-Plus 评测后的比赛目录，结构如下（`src/`、`result/` 会被忽略）：

```
<比赛目录>/
├── .ccr                    # 题目顺序（可选，XML 格式）
├── data/
│   ├── A/                  # 每道题一个目录
│   │   ├── .prb            # 题目配置（XML，未配置过的比赛可能没有）
│   │   ├── A1.in / A1.out  # 测试点数据
│   │   └── ...
│   ├── B/
│   ├── C/
│   └── D/
├── src/                    # 选手代码（本工具忽略）
└── result/                 # 评测结果（本工具忽略）
```

## 支持的输入格式

CCR-Plus 比赛目录（或其 zip 打包）：

- **题目顺序**：来自比赛目录下的 `.ccr` 文件（XML）；缺失时按目录名字母序排列。
- **题目识别**：`data/` 下每道题一个目录，目录内的 `.prb` 文件（XML）记录题目配置；没有 `.prb` 的目录不会被判为题目。
- **测试点配对**：输入文件扩展名 `.in`/`.inp`（含无点形式），输出文件扩展名 `.out`/`.ans`/`.sol`/`.res`/`.std` 等，前后缀一致即配对成一对测试点。
- **每测试点独立限制**：`.prb` 中每个测试点可单独设置时限（秒）与内存（MB），本工具统一取最大值，并在各点不一致时给出警告。
- **自动降级**：未配置 `.prb` 的目录按传统题降级解析（默认 fulltext 校验、时限/内存取默认值 1s/128MB），并在报告中提示"该目录未在 CCR 中配置"。

## 支持的输出格式

### UOJ 经典格式

扁平 zip（**不要子目录**），根层级包含 `problem.conf` 与全部测试点文件。生成的 `problem.conf` 各字段：

| 字段 | 含义 |
|---|---|
| `use_builtin_judger` | 固定 `on`，使用 UOJ 内置评测器 |
| `use_builtin_checker` | 比较器：CCR `fulltext`（过滤行末空格及文末回车）映射为 `lcmp`（逐行比较）；可在界面改为 `wcmp`/`ncmp`/`rcmp4` 等 |
| `n_tests` | 测试点数量 |
| `n_ex_tests` / `n_sample_tests` | 固定 `0`（本工具不生成额外/样例测试点，见已知限制） |
| `input_pre` / `input_suf` | 输入文件名前缀/后缀，如 `A` + `in` → `A1.in` |
| `output_pre` / `output_suf` | 输出文件名前缀/后缀 |
| `time_limit` | 时限（秒，整数）；各测试点不一致时取最大值并上取整 |
| `memory_limit` | 内存限制（MB） |
| `output_limit` | 输出限制（MB），固定 `64` |

### Hydro 格式

zip 内保留一层题名文件夹：

```
<题名>/
├── problem.yaml            # 题目元信息（title 等）
├── problem_zh.md           # Markdown 题面（占位模板，需人工补充）
└── testdata/
    ├── config.yaml         # 测试数据配置
    ├── 1.in / 1.out        # 重命名后的测试点
    └── ...
```

`testdata/config.yaml` 关键字段：

| 字段 | 含义 |
|---|---|
| `type` | `default`（传统题） |
| `time` | 时限，毫秒整数（各点不一致时取最大值） |
| `memory` | 内存限制（MB） |
| `filename` | 文件 IO 前缀（来自 CCR 的 `<task input>`，去掉 `.in`；标准 IO 则无此项） |
| `checker_type` | CCR `fulltext` 映射为 `default`；有自定义校验器源码时为 `ccr`（Hydro 内置支持，可直接复用） |
| `cases` | 显式列出每个测试点的 input/output |

## 格式映射总表

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

## 已知限制与未来工作

- 不映射 Hydro subtask 分值（D8）——数据在 `.prb` 中已有，未来可加 `subtasks:` 输出
- 不生成/推断 std.cpp、val.cpp（UOJ hack 功能需人工补充）
- 题面不转换（D10）；Hydro 仅占位模板
- UOJ 自定义校验器无法自动转换（接口不同，0.7 表）
- 提交答案题/交互题转换（UOJ submit_answer / interactive 配置）
