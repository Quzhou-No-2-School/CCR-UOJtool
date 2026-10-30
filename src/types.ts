//! 前端类型定义：与 src-tauri 侧 serde DTO 一一对应。
//! 字段名以 src-tauri/src/engine/model.rs 与 src-tauri/src/engine/report.rs 为准。

/** CCR 题目类型（对应 `.prb` 根节点 `type` 属性）。 */
export type ProblemKind = "Traditional" | "AnswersOnly" | "Interactive" | "Unknown";

/** 单个测试点（model.rs `TestCaseDto`，文件相对 `data/<题名>/`）。 */
export interface TestCaseDto {
  in_file: string;
  out_file: string;
  /** 时限（秒，可为小数）。 */
  time_limit: number;
  /** 内存限制（MB）。 */
  memory_limit: number;
  /** 所属 subtask 分值。 */
  score: number;
}

/** 一道题（model.rs `ProblemDto`，附带引擎推断的统一限制）。 */
export interface ProblemDto {
  name: string;
  kind: ProblemKind;
  /** `<task input>`，如 "A.in"。 */
  in_file: string | null;
  /** `<task output>`，如 "A.out"。 */
  out_file: string | null;
  /** 内置校验器名（去扩展名后，如 "fulltext"）。 */
  checker: string | null;
  /** data 目录中的自定义校验器文件名（若有）。 */
  custom_checker: string | null;
  cases: TestCaseDto[];
  warnings: string[];
  prb_found: boolean;
  /** 统一时限（秒，各点最大值；无测试点时为 f64::MIN）。 */
  max_time: number;
  /** 统一内存（MB，各点最大值）。 */
  max_mem: number;
  uniform_score: boolean;
}

/** 一场比赛（model.rs `ContestDto`）。 */
export interface ContestDto {
  name: string;
  path: string;
  problems: ProblemDto[];
}

/** 每题用户编辑（model.rs `ProblemEditDto`）。 */
export interface ProblemEditDto {
  /** 对应题目目录名。 */
  name: string;
  /** 覆盖题号（UOJ input_pre / Hydro filename）。 */
  target_id: string | null;
  /** 覆盖统一时限（秒）。 */
  time_limit: number | null;
  /** 覆盖统一内存（MB）。 */
  memory_limit: number | null;
  /** 覆盖 checker（UOJ use_builtin_checker / Hydro checker_type）。 */
  checker: string | null;
  /** 是否导出该题。 */
  enabled: boolean;
}

/** 单题导出报告（report.rs `ProblemReport`）。 */
export interface ProblemReport {
  problem: string;
  /** "uoj" / "hydro"；未导出时为 "skipped"。 */
  target: string;
  /** 产出的 zip 路径；跳过或失败时为 null。 */
  zip_path: string | null;
  n_tests: number;
  skipped: boolean;
  warnings: string[];
  errors: string[];
}

/** 整场导出报告（report.rs `ExportReport`）。 */
export interface ExportReport {
  items: ProblemReport[];
}

/** 导出目标（commands.rs `export_problems` 的 target 参数）。 */
export type ExportTarget = "uoj" | "hydro" | "both";

/** 详情编辑器中每题的编辑状态（字符串形式，提交导出时解析为 ProblemEditDto）。 */
export interface ProblemEditState {
  /** 覆盖题号。 */
  targetId: string;
  /** 统一时限（秒，输入框原文）。 */
  time: string;
  /** 统一内存（MB，输入框原文）。 */
  mem: string;
  /** checker 覆盖。 */
  checker: string;
  /** 是否导出。 */
  enabled: boolean;
}
