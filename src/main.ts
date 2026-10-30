//! 前端入口：状态管理、IPC 调用与界面渲染（vanilla TS，无框架）。
//! 全部节点用 DOM API 构建，不使用 innerHTML，避免不可信内容注入。

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { ContestDto, ProblemDto } from "./types";

// ---- 全局状态 ----

let contest: ContestDto | null = null;
let outDir: string | null = null;
let selectedName: string | null = null;

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
  sec.replaceChildren(el("p", "placeholder", `${p.name}：详情编辑（开发中）`));
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
