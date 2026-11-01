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
