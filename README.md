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
