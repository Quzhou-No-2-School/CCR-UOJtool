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
