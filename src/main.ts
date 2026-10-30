//! 前端入口：状态管理、IPC 调用与界面渲染（vanilla TS，无框架）。
//! 全部节点用 DOM API 构建，不使用 innerHTML，避免不可信内容注入。

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { ContestDto, ProblemDto, ProblemEditState } from "./types";

// ---- 全局状态 ----

let contest: ContestDto | null = null;
let outDir: string | null = null;
let selectedName: string | null = null;
/** 每题编辑状态，键为题目目录名。 */
const edits = new Map<string, ProblemEditState>();

const $ = <T extends HTMLElement = HTMLElement>(sel: string): T => {
  const el = document.querySelector<T>(sel);
  if (!el) throw new Error(`页面缺少元素：${sel}`);
  return el;
};

/** 创建元素的便捷方法（textContent 赋值，天然免注入）。 */
function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className !== undefined) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

// ---- 工具 ----

let toastTimer: number | undefined;

function toast(message: string, kind: "error" | "info" = "error"): void {
  const el = $("#toast");
  el.textContent = message;
  el.className = kind;
  el.hidden = false;
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    el.hidden = true;
  }, 6000);
}

/** 操作期间禁用按钮，防重复点击。 */
async function withBusy(btn: HTMLButtonElement, fn: () => Promise<void>): Promise<void> {
  btn.disabled = true;
  try {
    await fn();
  } finally {
    btn.disabled = false;
  }
}

function isConvertible(p: ProblemDto): boolean {
  return p.kind === "Traditional";
}

function kindLabel(kind: ProblemDto["kind"]): string {
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

function fmtNum(v: number): string {
  if (!Number.isFinite(v)) return "—";
  return Number.isInteger(v) ? String(v) : String(Number(v.toFixed(3)));
}

/** values 的 min~max 区间；空数组返回 "—"。 */
function fmtRange(values: number[]): string {
  if (!values.length) return "—";
  const min = Math.min(...values);
  const max = Math.max(...values);
  return min === max ? fmtNum(min) : `${fmtNum(min)}~${fmtNum(max)}`;
}

// ---- 每题编辑状态 ----

/** 默认编辑值取引擎推断：target_id=题名、time/mem=各点最大值上取整、
 * checker=fulltext 的保守映射 lcmp（src-tauri/src/engine/uoj.rs map_checker）。
 * 无测试点时与后端回退一致（1s / 128MB）。 */
function defaultEdit(p: ProblemDto): ProblemEditState {
  return {
    targetId: p.name,
    time: p.cases.length ? String(Math.ceil(p.max_time)) : "1",
    mem: p.cases.length ? String(Math.ceil(p.max_mem)) : "128",
    checker: "lcmp",
    enabled: true,
  };
}

function initEdits(): void {
  edits.clear();
  if (contest) {
    for (const p of contest.problems) edits.set(p.name, defaultEdit(p));
  }
}

// ---- 状态栏与按钮可用性 ----

function updateStatus(): void {
  $("#contest-info").textContent = contest
    ? `比赛：${contest.name} — ${contest.path}`
    : "未打开比赛：请先打开比赛目录或导入比赛 zip";
  $("#out-dir-info").textContent = `输出目录：${outDir ?? "未选择"}`;
  const ready = contest !== null && outDir !== null;
  $<HTMLButtonElement>("#btn-export-uoj").disabled = !ready;
  $<HTMLButtonElement>("#btn-export-hydro").disabled = !ready;
  $<HTMLButtonElement>("#btn-export-both").disabled = !ready;
}

// ---- 题目列表 ----

function badge(className: string, text: string): HTMLSpanElement {
  return el("span", `badge ${className}`, text);
}

function renderProblemList(): void {
  const list = $("#problem-list");
  list.replaceChildren();
  if (!contest) {
    list.appendChild(el("p", "placeholder", "未打开比赛：请先打开比赛目录或导入比赛 zip。"));
    return;
  }
  for (const p of contest.problems) {
    const conv = isConvertible(p);
    const card = el("button", "problem-card");
    card.type = "button";
    if (p.name === selectedName) card.classList.add("selected");
    if (!conv) card.classList.add("unsupported");

    const title = el("div", "card-title");
    title.appendChild(el("span", "problem-name", p.name));
    title.appendChild(badge(`badge-kind-${p.kind}`, kindLabel(p.kind)));
    card.appendChild(title);

    const meta = el("div", "card-meta");
    meta.appendChild(el("span", undefined, `${p.cases.length} 个测试点`));
    meta.appendChild(el("span", undefined, `时限 ${fmtRange(p.cases.map((c) => c.time_limit))}s`));
    meta.appendChild(el("span", undefined, `内存 ${fmtRange(p.cases.map((c) => c.memory_limit))}MB`));
    card.appendChild(meta);

    const extra = el("div", "card-extra");
    extra.appendChild(el("span", "card-checker", `checker: ${p.custom_checker ?? p.checker ?? "无"}`));
    if (!conv) extra.appendChild(badge("badge-unsupported", "V1 不支持转换"));
    if (p.warnings.length > 0) extra.appendChild(badge("badge-warn", `⚠ ${p.warnings.length}`));
    card.appendChild(extra);

    card.addEventListener("click", () => {
      selectedName = p.name;
      renderProblemList();
      renderDetail();
    });
    list.appendChild(card);
  }
}

// ---- 题目详情 ----

/** 只读测试点映射表。targetIdForNames 为 null 时不显示新文件名列。 */
function appendCaseTable(sec: HTMLElement, p: ProblemDto, targetIdForNames: string | null): void {
  sec.appendChild(el("h3", "section-title", `测试点映射（${p.cases.length} 个）`));
  if (!p.cases.length) {
    sec.appendChild(el("p", "placeholder", "该题没有测试点。"));
    return;
  }
  const tbody = el("tbody");
  const thead = el("thead");
  const hr = el("tr");
  for (const h of ["#", "输入文件", "输出文件"]) hr.appendChild(el("th", undefined, h));
  if (targetIdForNames !== null) {
    hr.appendChild(el("th", undefined, "UOJ 文件名 (in/out)"));
    hr.appendChild(el("th", undefined, "Hydro 文件名 (in/out)"));
  }
  for (const h of ["时限 (s)", "内存 (MB)", "分值"]) hr.appendChild(el("th", undefined, h));
  thead.appendChild(hr);

  p.cases.forEach((c, i) => {
    const tr = el("tr");
    tr.appendChild(el("td", "num", String(i + 1)));
    tr.appendChild(el("td", undefined, c.in_file));
    tr.appendChild(el("td", undefined, c.out_file));
    if (targetIdForNames !== null) {
      // 新文件名规则与后端一致：UOJ `<target_id><i>.in/.out`（uoj.rs）、
      // Hydro `<target_id>/testdata/<i>.in/.out`（hydro.rs），i 从 1。
      const uojTd = el("td", undefined, `${targetIdForNames}${i + 1}.in / .out`);
      uojTd.dataset.mapUoj = String(i);
      tr.appendChild(uojTd);
      const hydroTd = el("td", undefined, `${targetIdForNames}/testdata/${i + 1}.in / .out`);
      hydroTd.dataset.mapHydro = String(i);
      tr.appendChild(hydroTd);
    }
    tr.appendChild(el("td", "num", fmtNum(c.time_limit)));
    tr.appendChild(el("td", "num", fmtNum(c.memory_limit)));
    tr.appendChild(el("td", "num", String(c.score)));
    tbody.appendChild(tr);
  });

  const table = el("table", "case-table");
  table.appendChild(thead);
  table.appendChild(tbody);
  sec.appendChild(table);
}

function addFormRow(
  form: HTMLElement,
  labelText: string,
  inputId: string,
  type: "text" | "number",
  value: string,
  onInput: (v: string) => void,
): HTMLInputElement {
  const label = el("label", undefined, labelText);
  label.htmlFor = inputId;
  const input = document.createElement("input");
  input.type = type;
  input.id = inputId;
  input.value = value;
  if (type === "number") input.step = "any";
  input.addEventListener("input", () => onInput(input.value));
  form.appendChild(label);
  form.appendChild(input);
  return input;
}

function renderDetail(): void {
  const sec = $("#problem-detail");
  if (!contest) {
    sec.replaceChildren(el("p", "placeholder", "打开比赛后，在这里查看和编辑题目。"));
    return;
  }
  const p = contest.problems.find((x) => x.name === selectedName);
  if (!p) {
    sec.replaceChildren(el("p", "placeholder", "← 从左侧选择一道题。"));
    return;
  }
  const e = edits.get(p.name) ?? defaultEdit(p);
  edits.set(p.name, e);
  const conv = isConvertible(p);

  sec.replaceChildren();

  const title = el("div", "detail-title");
  title.appendChild(el("h2", undefined, p.name));
  title.appendChild(badge(`badge-kind-${p.kind}`, kindLabel(p.kind)));
  if (!conv) title.appendChild(badge("badge-unsupported", "V1 不支持转换"));
  sec.appendChild(title);

  if (p.warnings.length > 0) {
    const ul = el("ul", "warn-list");
    for (const w of p.warnings) ul.appendChild(el("li", undefined, w));
    sec.appendChild(ul);
  }

  if (!conv) {
    sec.appendChild(
      el("p", "unsupported-note", `${kindLabel(p.kind)}在 V1 不支持转换，导出时将自动跳过（仅出现在报告中）。`),
    );
    appendCaseTable(sec, p, null);
    return;
  }

  // 编辑表单（ANS/INT 不显示）
  const form = el("div", "edit-form");
  const targetInput = addFormRow(form, "导出题号", "edit-target-id", "text", e.targetId, (v) => {
    e.targetId = v;
    const id = v.trim() || p.name;
    for (const td of form.parentElement?.querySelectorAll<HTMLElement>("[data-map-uoj]") ?? []) {
      td.textContent = `${id}${Number(td.dataset.mapUoj) + 1}.in / .out`;
    }
    for (const td of form.parentElement?.querySelectorAll<HTMLElement>("[data-map-hydro]") ?? []) {
      td.textContent = `${id}/testdata/${Number(td.dataset.mapHydro) + 1}.in / .out`;
    }
  });
  targetInput.placeholder = p.name;
  addFormRow(form, "时限 (秒)", "edit-time", "number", e.time, (v) => {
    e.time = v;
  });
  addFormRow(form, "内存 (MB)", "edit-mem", "number", e.mem, (v) => {
    e.mem = v;
  });
  const checkerInput = addFormRow(form, "checker 覆盖", "edit-checker", "text", e.checker, (v) => {
    e.checker = v;
  });
  checkerInput.placeholder = "lcmp（fulltext 的默认映射）";

  const enabledRow = el("div", "checkbox-row");
  enabledRow.style.gridColumn = "1 / -1";
  const enabledInput = document.createElement("input");
  enabledInput.type = "checkbox";
  enabledInput.id = "edit-enabled";
  enabledInput.checked = e.enabled;
  enabledInput.addEventListener("change", () => {
    e.enabled = enabledInput.checked;
  });
  const enabledLabel = el("label", undefined, "导出该题");
  enabledLabel.htmlFor = "edit-enabled";
  enabledRow.appendChild(enabledInput);
  enabledRow.appendChild(enabledLabel);
  form.appendChild(enabledRow);
  sec.appendChild(form);

  sec.appendChild(
    el(
      "p",
      "card-checker",
      "默认值取引擎推断：题号=目录名，时限/内存=各点最大值上取整，checker=fulltext 映射 lcmp。留空时限/内存则由后端自行推断。",
    ),
  );

  appendCaseTable(sec, p, e.targetId.trim() || p.name);
}

// ---- 打开 / 导入比赛 ----

async function openContestDir(): Promise<void> {
  const path: string | null = await open({
    directory: true,
    multiple: false,
    title: "选择 CCR-Plus 比赛目录",
  });
  if (!path) return;
  try {
    contest = await invoke<ContestDto>("scan_contest", { path });
    selectedName = contest.problems[0]?.name ?? null;
    initEdits();
    renderProblemList();
    renderDetail();
    updateStatus();
  } catch (err) {
    toast(`扫描比赛失败：${String(err)}`);
  }
}

async function importContestZip(): Promise<void> {
  const zipPath: string | null = await open({
    multiple: false,
    title: "选择 CCR-Plus 比赛 zip 包",
    filters: [{ name: "CCR 比赛", extensions: ["zip"] }],
  });
  if (!zipPath) return;
  try {
    contest = await invoke<ContestDto>("import_contest_zip", { zipPath });
    selectedName = contest.problems[0]?.name ?? null;
    initEdits();
    renderProblemList();
    renderDetail();
    updateStatus();
    toast(`已导入比赛：${contest.name}`, "info");
  } catch (err) {
    toast(`导入比赛 zip 失败：${String(err)}`);
  }
}

async function chooseOutDir(): Promise<void> {
  const dir: string | null = await open({
    directory: true,
    multiple: false,
    title: "选择输出目录",
  });
  if (!dir) return;
  outDir = dir;
  updateStatus();
}

// ---- 入口 ----

function init(): void {
  $<HTMLButtonElement>("#btn-open-dir").addEventListener("click", () =>
    void withBusy($<HTMLButtonElement>("#btn-open-dir"), openContestDir),
  );
  $<HTMLButtonElement>("#btn-import-zip").addEventListener("click", () =>
    void withBusy($<HTMLButtonElement>("#btn-import-zip"), importContestZip),
  );
  $<HTMLButtonElement>("#btn-choose-out").addEventListener("click", () =>
    void withBusy($<HTMLButtonElement>("#btn-choose-out"), chooseOutDir),
  );
  renderProblemList();
  renderDetail();
  updateStatus();
}

init();
