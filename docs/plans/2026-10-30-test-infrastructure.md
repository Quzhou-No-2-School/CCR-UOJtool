# 多平台测试基础设施（编译测试 + Unit Test + E2E）实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为 CCR-UOJtool 建立覆盖前端与 Rust 两侧、在 Windows + Linux 双平台运行的测试体系（编译门禁、单元测试、引擎级 e2e），并接入 GitHub Actions。

**Architecture:** 前端从 545 行单体的 `main.ts` 中提取可独立测试的纯函数到 `src/lib.ts`，用 vitest 做单元测试（测试文件统一放根目录 `test/`）；Rust 侧测试已齐备（5 个文件内联单测 + `src-tauri/tests/` 下 3 个集成测试文件，含 `export_e2e.rs` 引擎级 e2e）。CI 用 GitHub Actions 显式两个 job 表达用户指定的依赖树：

- **Windows job**：编译测试（npm build + cargo fmt + clippy）→ unit test → e2e（步骤顺序即依赖，编译失败则后续不执行）
- **Linux job**：unit test → e2e（编译由 cargo test 隐式覆盖，无独立编译门禁）

**Tech Stack:** TypeScript 6 / Vite 8 / vitest 5（peer 兼容 vite ^8，已核实 `npm view vitest@latest peerDependencies`）；Rust 1.98 edition 2024 / cargo test；GitHub Actions（dtolnay/rust-toolchain、Swatinem/rust-cache、actions/setup-node）。

---

## 基线验证结论（2026-10-30 实测）

| 检查项 | 命令 | 结果 |
|---|---|---|
| 前端编译 | `npm run build` | ✅ 绿 |
| Rust clippy | `cargo clippy --all-targets -- -D warnings` | ✅ 绿 |
| Rust fmt | `cargo fmt --check` | ❌ 有 diff（工具链升级导致 import 排序变化），需先跑一次 `cargo fmt` |
| Rust 测试 | `cargo test` | 仓库内已有，纳入 CI 即可 |
| 前端测试 | 无 | 需新建（vitest） |

## 任务依赖与并行性

- **泳道 A**：Task 1（Rust fmt，只碰 `src-tauri/`）
- **泳道 B**：Task 2 → Task 3（前端提取 + vitest，只碰 `src/` 与 `package.json`）
- **汇合**：Task 4（CI workflow，依赖 1–3 全部落定后验证）、Task 5（全量验证 + 推送）

两条泳道文件区域零重叠，可并行推进。

约定：npm 命令在仓库根目录运行；cargo 命令在 `src-tauri/` 下运行（或在根目录加 `--manifest-path src-tauri/Cargo.toml`）。每个任务结束必须 commit，commit 信息用 conventional commits（仓库现行风格）。

---

### Task 1: 一次性 `cargo fmt` 对齐（泳道 A）

**Files:**
- Modify: `src-tauri/src/**/*.rs`（仅格式，无逻辑变更）

**Step 1: 运行格式化**

```bash
cd src-tauri && cargo fmt
```

**Step 2: 验证格式与编译门禁**

```bash
cargo fmt --check && echo FMT-OK
cargo clippy --all-targets -- -D warnings 2>&1 | tail -2
```

Expected: 输出 `FMT-OK`；clippy 输出 `Finished`（无 warning）。

**Step 3: Commit**

```bash
git add -A src-tauri/src
git commit -m "style: apply rustfmt"
```

---

### Task 2: 提取前端纯函数到 `src/lib.ts`（泳道 B）

**Files:**
- Create: `src/lib.ts`
- Modify: `src/main.ts:1-13`（import 区）、`src/main.ts:66-111`（删除被移函数）、`src/main.ts:389-447`（删除被移函数）、`src/main.ts:451,456`（两处调用点加实参）

**Step 1: 创建 `src/lib.ts`**

完整内容（函数体从 `main.ts` 原样搬移，仅 `validateEdits`/`buildEditsPayload` 把全局 `contest`/`edits` 改为参数；`ReadonlyMap` 兼容调用方传入的 `Map`）：

```ts
//! 可独立测试的纯函数：格式化、标签、编辑状态推导与校验。
//! 不依赖 DOM 与 Tauri IPC，供 main.ts 与 vitest 共用。

import type {
  ContestDto,
  ExportTarget,
  ProblemDto,
  ProblemEditDto,
  ProblemEditState,
} from "./types";

export function isConvertible(p: ProblemDto): boolean {
  return p.kind === "Traditional";
}

export function kindLabel(kind: ProblemDto["kind"]): string {
  switch (kind) {
    case "Traditional":
      return "传统题";
    case "AnswersOnly":
      return "提交答案题";
    case "Interactive":
      return "交互题";
    default:
      return "未知类型";
  }
}

export function targetLabel(target: ExportTarget): string {
  switch (target) {
    case "uoj":
      return "UOJ";
    case "hydro":
      return "Hydro";
    case "both":
      return "UOJ + Hydro";
  }
}

export function fmtNum(v: number): string {
  if (!Number.isFinite(v)) return "—";
  return Number.isInteger(v) ? String(v) : String(Number(v.toFixed(3)));
}

/** values 的 min~max 区间；空数组返回 "—"。 */
export function fmtRange(values: number[]): string {
  if (!values.length) return "—";
  const min = Math.min(...values);
  const max = Math.max(...values);
  return min === max ? fmtNum(min) : `${fmtNum(min)}~${fmtNum(max)}`;
}

/** 默认编辑值取引擎推断：target_id=题名、time/mem=各点最大值上取整。
 * checker 留空 → payload 发 null → 后端按目标各自映射（UOJ fulltext→lcmp；
 * Hydro 自定义校验器→ccr，否则 default）。切勿在 UI 侧硬编码映射结果：
 * 同一个 checker 值会被后端同时用作 UOJ/Hydro 的覆盖，lcmp 对 Hydro 非法。
 * 无测试点时与后端回退一致（1s / 128MB）。 */
export function defaultEdit(p: ProblemDto): ProblemEditState {
  return {
    targetId: p.name,
    time: p.cases.length ? String(Math.ceil(p.max_time)) : "1",
    mem: p.cases.length ? String(Math.ceil(p.max_mem)) : "128",
    checker: "",
    enabled: true,
  };
}

/** 校验可导出题的时限/内存输入；非法时返回错误描述。 */
export function validateEdits(
  contest: ContestDto | null,
  edits: ReadonlyMap<string, ProblemEditState>,
): string | null {
  if (!contest) return "尚未打开比赛";
  for (const p of contest.problems) {
    const e = edits.get(p.name);
    if (!isConvertible(p) || !e?.enabled) continue;
    for (const [label, raw] of [
      ["时限", e.time],
      ["内存", e.mem],
    ] as const) {
      const s = raw.trim();
      if (s === "") continue;
      const n = Number(s);
      if (!Number.isFinite(n) || n <= 0) {
        return `${p.name}：${label}“${s}”不是有效的正数`;
      }
    }
  }
  return null;
}

/** 把编辑状态转成 ProblemEditDto；空字符串 → null（由后端自行推断）。 */
export function buildEditsPayload(
  contest: ContestDto,
  edits: ReadonlyMap<string, ProblemEditState>,
): ProblemEditDto[] {
  return contest.problems.map((p) => {
    const e = edits.get(p.name) ?? defaultEdit(p);
    const parseLimit = (raw: string): number | null => {
      const s = raw.trim();
      if (s === "") return null;
      const n = Number(s);
      return Number.isFinite(n) && n > 0 ? n : null;
    };
    return {
      name: p.name,
      target_id: e.targetId.trim() || null,
      time_limit: parseLimit(e.time),
      memory_limit: parseLimit(e.mem),
      checker: e.checker.trim() || null,
      enabled: e.enabled,
    };
  });
}
```

**Step 2: 修改 `src/main.ts`**

1. import 区（`main.ts:1-13`）替换为：

```ts
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  buildEditsPayload,
  defaultEdit,
  fmtNum,
  fmtRange,
  isConvertible,
  kindLabel,
  targetLabel,
  validateEdits,
} from "./lib";
import type { ContestDto, ExportReport, ExportTarget, ProblemEditState } from "./types";
```

（`ProblemDto`、`ProblemEditDto` 从 import 中删除——搬移后 `main.ts` 不再直接引用，`noUnusedLocals` 会报错。）

2. 删除已搬走的函数定义：`isConvertible`、`kindLabel`、`fmtNum`、`fmtRange`（原 66-94 行）、`defaultEdit`（原 103-111 行）、`targetLabel`（原 389-398 行）、`validateEdits`（原 407-425 行）、`buildEditsPayload`（原 428-447 行）。注意保留 `initEdits`（它调用 `defaultEdit`，签名不变）。

3. `exportContest` 内两处调用点加实参：

```ts
const invalid = validateEdits(contest, edits);
// ...
const payload = buildEditsPayload(contest, edits);
```

**Step 3: 编译验证**

```bash
npm run build
```

Expected: tsc 无错误、vite 构建成功（`✓ built in ...`）。

**Step 4: Commit**

```bash
git add src/lib.ts src/main.ts
git commit -m "refactor(ui): extract testable pure functions into lib"
```

---

### Task 3: 安装 vitest 并编写 `test/lib.test.ts`（泳道 B）

**Files:**
- Modify: `package.json`（devDependencies + scripts.test）
- Create: `test/lib.test.ts`（根目录 `test/` 文件夹，用户指定）

> 测试文件位置说明：前端测试统一放根目录 `test/`；Rust 集成/e2e 已在 `src-tauri/tests/`（cargo 强制约定）；Rust 内联 `#[cfg(test)]` 单测为语言机制，无法移到独立文件夹。`test/` 不在 tsconfig `include: ["src"]` 内，故 `npm run build` 的 tsc 不检查测试文件（构建无测试依赖），vitest 运行时自行转译。

**Step 1: 安装 vitest（当前 latest 5.0.5，peer 兼容 vite ^8）**

```bash
npm install -D vitest
```

**Step 2: `package.json` scripts 增加**

```json
"test": "vitest run"
```

（零配置即可：vitest 默认发现 `**/*.test.ts`（含根目录 `test/`），会加载现有 `vite.config.ts` 但其内容只影响 dev server，无碍。）

**Step 3: 编写 `test/lib.test.ts`（完整内容；注意 import 路径为 `../src/`）**

```ts
//! src/lib.ts 纯函数的单元测试。位于根目录 test/ 下。

import { describe, expect, it } from "vitest";
import {
  buildEditsPayload,
  defaultEdit,
  fmtNum,
  fmtRange,
  isConvertible,
  kindLabel,
  targetLabel,
  validateEdits,
} from "../src/lib";
import type { ContestDto, ProblemDto, ProblemEditState, TestCaseDto } from "../src/types";

// ---- 夹具 ----

function mkCase(inFile: string, time = 1, mem = 128): TestCaseDto {
  return {
    in_file: inFile,
    out_file: inFile.replace(/\.in$/, ".out"),
    time_limit: time,
    memory_limit: mem,
    score: 100,
  };
}

function mkProblem(overrides: Partial<ProblemDto> = {}): ProblemDto {
  return {
    name: "A",
    kind: "Traditional",
    in_file: "A.in",
    out_file: "A.out",
    checker: "fulltext",
    custom_checker: null,
    cases: [mkCase("A1.in")],
    warnings: [],
    prb_found: true,
    max_time: 1,
    max_mem: 128,
    uniform_score: true,
    ...overrides,
  };
}

function mkContest(problems: ProblemDto[]): ContestDto {
  return { name: "contest", path: "/tmp/c", problems };
}

function mkEdit(overrides: Partial<ProblemEditState> = {}): ProblemEditState {
  return { targetId: "A", time: "1", mem: "128", checker: "", enabled: true, ...overrides };
}

// ---- 格式化 ----

describe("fmtNum", () => {
  it("整数原样输出", () => expect(fmtNum(5)).toBe("5"));
  it("小数保留至多 3 位并去尾零", () => expect(fmtNum(1.5)).toBe("1.5"));
  it("浮点误差经 toFixed 归一", () => expect(fmtNum(0.1 + 0.2)).toBe("0.3"));
  it("非有限值返回占位符", () => {
    expect(fmtNum(Number.NaN)).toBe("—");
    expect(fmtNum(Number.POSITIVE_INFINITY)).toBe("—");
  });
});

describe("fmtRange", () => {
  it("空数组返回占位符", () => expect(fmtRange([])).toBe("—"));
  it("单值区间退化为单数", () => expect(fmtRange([2])).toBe("2"));
  it("区间用波浪线连接", () => expect(fmtRange([1, 3, 2])).toBe("1~3"));
  it("小数区间", () => expect(fmtRange([0.5, 1.25])).toBe("0.5~1.25"));
});

// ---- 标签 ----

describe("标签函数", () => {
  it("isConvertible 仅传统题可转换", () => {
    expect(isConvertible(mkProblem())).toBe(true);
    expect(isConvertible(mkProblem({ kind: "AnswersOnly" }))).toBe(false);
    expect(isConvertible(mkProblem({ kind: "Interactive" }))).toBe(false);
    expect(isConvertible(mkProblem({ kind: "Unknown" }))).toBe(false);
  });
  it("kindLabel 覆盖全部类型", () => {
    expect(kindLabel("Traditional")).toBe("传统题");
    expect(kindLabel("AnswersOnly")).toBe("提交答案题");
    expect(kindLabel("Interactive")).toBe("交互题");
    expect(kindLabel("Unknown")).toBe("未知类型");
  });
  it("targetLabel 覆盖全部导出目标", () => {
    expect(targetLabel("uoj")).toBe("UOJ");
    expect(targetLabel("hydro")).toBe("Hydro");
    expect(targetLabel("both")).toBe("UOJ + Hydro");
  });
});

// ---- 编辑状态 ----

describe("defaultEdit", () => {
  it("有测试点时取各点最大值上取整", () => {
    const p = mkProblem({
      cases: [mkCase("A1.in", 1, 256), mkCase("A2.in", 2.5, 128)],
      max_time: 2.5,
      max_mem: 256,
    });
    expect(defaultEdit(p)).toEqual({
      targetId: "A",
      time: "3",
      mem: "256",
      checker: "",
      enabled: true,
    });
  });
  it("无测试点时回退 1s / 128MB（与后端一致）", () => {
    expect(defaultEdit(mkProblem({ cases: [] }))).toEqual({
      targetId: "A",
      time: "1",
      mem: "128",
      checker: "",
      enabled: true,
    });
  });
});

describe("validateEdits", () => {
  it("未打开比赛时报错", () => {
    expect(validateEdits(null, new Map())).toBe("尚未打开比赛");
  });
  it("默认编辑通过", () => {
    expect(validateEdits(mkContest([mkProblem()]), new Map([["A", mkEdit()]]))).toBeNull();
  });
  it("空字符串表示交由后端推断，通过", () => {
    expect(
      validateEdits(mkContest([mkProblem()]), new Map([["A", mkEdit({ time: "", mem: "" })]])),
    ).toBeNull();
  });
  it("非法时限报错并含题名", () => {
    const err = validateEdits(mkContest([mkProblem()]), new Map([["A", mkEdit({ time: "abc" })]]));
    expect(err).toContain("A");
    expect(err).toContain("时限");
  });
  it("零与负数非法", () => {
    const c = mkContest([mkProblem()]);
    expect(validateEdits(c, new Map([["A", mkEdit({ time: "0" })]]))).not.toBeNull();
    expect(validateEdits(c, new Map([["A", mkEdit({ mem: "-1" })]]))).not.toBeNull();
  });
  it("未勾选导出的题不校验", () => {
    const e = mkEdit({ time: "abc", enabled: false });
    expect(validateEdits(mkContest([mkProblem()]), new Map([["A", e]]))).toBeNull();
  });
  it("不可转换的题不校验", () => {
    const p = mkProblem({ kind: "Interactive" });
    expect(validateEdits(mkContest([p]), new Map([["A", mkEdit({ time: "abc" })]]))).toBeNull();
  });
});

describe("buildEditsPayload", () => {
  it("默认编辑映射为 DTO", () => {
    expect(buildEditsPayload(mkContest([mkProblem()]), new Map([["A", mkEdit()]]))).toEqual([
      { name: "A", target_id: "A", time_limit: 1, memory_limit: 128, checker: null, enabled: true },
    ]);
  });
  it("空题号/时限/内存/checker 归一为 null", () => {
    const e = mkEdit({ targetId: "  ", time: "", mem: "", checker: "" });
    expect(buildEditsPayload(mkContest([mkProblem()]), new Map([["A", e]]))).toEqual([
      { name: "A", target_id: null, time_limit: null, memory_limit: null, checker: null, enabled: true },
    ]);
  });
  it("非法数字时限归一为 null（validateEdits 在前的兜底）", () => {
    const dto = buildEditsPayload(mkContest([mkProblem()]), new Map([["A", mkEdit({ time: "abc" })]]))[0];
    expect(dto.time_limit).toBeNull();
  });
  it("题号与 checker 去除首尾空白", () => {
    const e = mkEdit({ targetId: " B1 ", checker: " lcmp " });
    const dto = buildEditsPayload(mkContest([mkProblem()]), new Map([["A", e]]))[0];
    expect(dto.target_id).toBe("B1");
    expect(dto.checker).toBe("lcmp");
  });
  it("编辑状态缺失时回退 defaultEdit", () => {
    const dto = buildEditsPayload(
      mkContest([mkProblem({ max_time: 2, max_mem: 512 })]),
      new Map(),
    )[0];
    expect(dto.time_limit).toBe(2);
    expect(dto.memory_limit).toBe(512);
  });
  it("多题保持输入顺序", () => {
    const c = mkContest([mkProblem({ name: "A" }), mkProblem({ name: "B" })]);
    expect(buildEditsPayload(c, new Map()).map((p) => p.name)).toEqual(["A", "B"]);
  });
});
```

**Step 4: 运行测试**

```bash
npm test
```

Expected: `Test Files  1 passed`，约 24 个用例全绿。

**Step 5: Commit**

```bash
git add package.json package-lock.json test/lib.test.ts
git commit -m "test(ui): vitest unit tests for lib pure functions"
```

---

### Task 4: GitHub Actions 多平台 CI workflow（依赖树：Windows 编译门禁 → unit/e2e；Linux 直接 unit/e2e）

> **执行调整（2026-10-30，用户要求）：** unit 与 e2e 拆分为 `needs:` 依赖的独立子 job，且两平台对称——最终结构为 `windows-compile → { windows-unit, windows-e2e }` 与 `linux-compile → { linux-unit, linux-e2e }`（Linux 编译门禁覆盖 webkit2gtk/glibc 等平台差异，失败时快速终止测试 job）。同平台 job 用 rust-cache `shared-key` 共享缓存避免重复全量编译；e2e 子 job 纯 Rust 无需 Node（已实测：缺 `dist/` 时 `cargo check` 正常通过）；Linux 的 unit/e2e 因链接期依赖仍需 apt 系统库。以下原文保留当时的设计（单 job 内步骤顺序表达依赖）。

**Files:**
- Create: `.github/workflows/ci.yml`

**Step 1: 编写 `.github/workflows/ci.yml`（完整内容）**

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

jobs:
  # 依赖树 1：Windows 平台 编译测试 -> {unit test 和 e2e}
  windows:
    runs-on: windows-latest
    defaults:
      run:
        shell: bash
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust stable
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Rust cache
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: src-tauri

      - name: Setup Node 22
        uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: npm

      - name: Install frontend dependencies
        run: npm ci

      # ---- 编译测试（门禁，失败则后续步骤不执行）----
      - name: Type-check & build frontend (tsc + vite)
        run: npm run build

      - name: Rust fmt
        run: cargo fmt --check
        working-directory: src-tauri

      - name: Rust clippy
        run: cargo clippy --all-targets -- -D warnings
        working-directory: src-tauri

      # ---- unit test ----
      - name: Frontend unit tests (vitest)
        run: npm test

      - name: Rust unit + integration tests
        run: cargo test --lib --tests
        working-directory: src-tauri

      # ---- e2e test（引擎级：真实夹具 → 导出 zip → 校验内容）----
      - name: Engine e2e tests
        run: cargo test --test export_e2e
        working-directory: src-tauri

  # 依赖树 2：Linux 平台 unit + e2e（编译由 cargo test 隐式覆盖）
  linux:
    runs-on: ubuntu-latest
    defaults:
      run:
        shell: bash
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust stable
        uses: dtolnay/rust-toolchain@stable

      - name: Rust cache
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: src-tauri

      - name: Setup Node 22
        uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: npm

      # tauri 依赖在 Linux 上链接 webkit2gtk，cargo test 编译期即需要
      - name: Install Linux system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential libssl-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev patchelf

      - name: Install frontend dependencies
        run: npm ci

      # ---- unit test ----
      - name: Frontend unit tests (vitest)
        run: npm test

      - name: Rust unit + integration tests
        run: cargo test --lib --tests
        working-directory: src-tauri

      # ---- e2e test ----
      - name: Engine e2e tests
        run: cargo test --test export_e2e
        working-directory: src-tauri
```

设计说明：
- 两个显式 job 对应用户指定的依赖树；job 内步骤顺序即依赖（任一步失败，同 job 后续步骤自动跳过）。
- Windows job 独有编译门禁（tsc/vite、fmt、clippy）——重点拦截 Windows 工具链编译破坏；Linux 侧编译由 `cargo test` 隐式覆盖。
- `cargo test --lib --tests` 与 `cargo test --test export_e2e` 分两步，报告里能直观区分单测/集成与 e2e（编译产物复用）。
- Windows 上 `cargo test` 编译 tauri 时 WebView2 SDK 自动下载，无需额外步骤；`defaults.shell: bash` 统一两平台脚本解释器。
- 不加 coverage 与 release 产物上传（本次范围只有测试）。

**Step 2: 本地 yaml 语法自检**

```bash
node -e "const y=require('js-yaml');y.load(require('fs').readFileSync('.github/workflows/ci.yml','utf8'));console.log('YAML-OK')"
```

若无 js-yaml 则跳过此步（语法错误会在推送后由 Actions 立刻暴露）。

**Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: multi-platform test workflow (windows + linux)"
```

---

### Task 5: 本地全量验证 + 推送开 PR

**Step 1: 本地按 CI 顺序全量跑一遍**

```bash
npm run build
(cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings)
npm test
(cd src-tauri && cargo test --lib --tests && cargo test --test export_e2e)
```

Expected: 全部绿。

**Step 2: 推送验证（需用户确认后执行）**

```bash
git push -u origin main   # 或推到 feature 分支开 PR
```

在 GitHub Actions 页面确认 `windows` 与 `linux` 两个 job 均绿（Windows 应先出现编译门禁步骤）。首跑两平台均需全量编译 Rust 依赖（约 5-10 分钟），后续有 rust-cache 加速。

---

## 明确不做（YAGNI）

- UI 级 e2e（tauri-driver + WebDriver）——已与用户确认砍掉，引擎级 e2e 覆盖核心转换链路
- 前端 coverage 统计、eslint / prettier 门禁
- release / 构建产物发布 workflow（README 中"计划通过 CI 自动发布"另起任务）
- Windows 专用路径断言等平台差异测试——现有 Rust 测试已用 `tempfile` 做平台无关夹具
