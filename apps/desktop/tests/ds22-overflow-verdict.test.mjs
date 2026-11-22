import { strict as assert } from "node:assert";
import { test } from "node:test";
import { join } from "node:path";
import { comparePending } from "../../../harness/ds-overflow/verdict.mjs";
import { checkSteps, dumpJustfile } from "../../../harness/check-list/list.mjs";
import { repoRoot } from "./paths.mjs";

const first = {
  story: "숫자/Money",
  id: "number-money--light-0",
  width: "narrow",
  scale: "100%",
  findings: [{ kind: "unauthorized-overflow", element: "section" }],
};
const second = {
  story: "기본/Button",
  id: "button--dark-2",
  width: "normal",
  scale: "125%",
  findings: [
    { kind: "clipped", element: "button" },
    { kind: "unauthorized-overflow", element: "div" },
  ],
};
const pending = [
  { ...first, reason: "숫자를 표시할 가로 공간의 배치 결정이 필요하다." },
  { ...second, reason: "버튼 높이와 줄바꿈의 겉모습 결정이 필요하다." },
];

test("only the exact pending conditions and findings pass", () => {
  assert.deepEqual(comparePending([first, second], pending), []);
  assert.deepEqual(comparePending([], []), []);
  assert.notDeepEqual(comparePending([first], pending), []);
  assert.notDeepEqual(comparePending([first, second], pending.slice(0, 1)), []);
  assert.notDeepEqual(
    comparePending([{ ...first, id: "another-story" }, second], pending),
    [],
  );
  assert.notDeepEqual(
    comparePending([{ ...first, scale: "125%" }, second], pending),
    [],
  );
  assert.notDeepEqual(
    comparePending([{ ...first, findings: [] }, second], pending),
    [],
  );
  assert.notDeepEqual(
    comparePending(
      [
        { ...first, findings: [{ kind: "clipped", element: "section" }] },
        second,
      ],
      pending,
    ),
    [],
  );
  assert.notDeepEqual(
    comparePending(
      [
        {
          ...first,
          findings: [{ kind: "unauthorized-overflow", element: "div" }],
        },
        second,
      ],
      pending,
    ),
    [],
  );
});

test("comparison preserves finding multiplicity but ignores ordering", () => {
  assert.deepEqual(
    comparePending(
      [{ ...second, findings: second.findings.toReversed() }, first],
      pending,
    ),
    [],
  );
  assert.notDeepEqual(
    comparePending(
      [
        first,
        { ...second, findings: [...second.findings, second.findings[0]] },
      ],
      pending,
    ),
    [],
  );
});

test("a finding moved between same-tag elements does not match the pending entry", () => {
  const original = {
    ...first,
    findings: [{ kind: "unauthorized-overflow", element: "p @ html > p:1" }],
  };
  const moved = {
    ...first,
    findings: [{ kind: "unauthorized-overflow", element: "p @ html > p:2" }],
  };
  assert.deepEqual(
    comparePending([original], [{ ...original, reason: "보류" }]),
    [],
  );
  assert.notDeepEqual(
    comparePending([moved], [{ ...original, reason: "보류" }]),
    [],
  );
});

test("missing reasons, invalid conditions, and duplicate conditions fail", () => {
  assert.notDeepEqual(
    comparePending(
      [first, second],
      [{ ...pending[0], reason: "" }, pending[1]],
    ),
    [],
  );
  assert.notDeepEqual(comparePending([first, first], [pending[0]]), []);
  assert.notDeepEqual(comparePending([first], [pending[0], pending[0]]), []);
  assert.notDeepEqual(
    comparePending([{ ...first, width: "unknown" }], pending),
    [],
  );
  assert.notDeepEqual(
    comparePending([first], [{ ...pending[0], scale: "150%" }]),
    [],
  );
  for (const invalid of [
    { ...first, width: "unknown" },
    { ...first, scale: "150%" },
    { ...first, findings: [] },
  ]) {
    assert.notDeepEqual(
      comparePending([invalid], [{ ...invalid, reason: "잘못된 조건" }]),
      [],
    );
  }
  const unavailable = {
    ...first,
    findings: [{ kind: "layout-unavailable", element: "html" }],
  };
  assert.notDeepEqual(
    comparePending([unavailable], [{ ...unavailable, reason: "분류 불가" }]),
    [],
  );
});

test("ds-check runs both the catalog contract and fixed-browser overflow checks", () => {
  const dump = dumpJustfile(join(repoRoot, "justfile"));
  const steps = checkSteps(dump, "ds-check").map((step) => step.command);
  assert.ok(steps.includes("node harness/ds-catalog/check.mjs"));
  assert.ok(steps.includes("node harness/ds-overflow/typecheck.mjs"));
  assert.ok(
    steps.includes(
      "node --test apps/desktop/tests/ds22-overflow-verdict.test.mjs",
    ),
  );
  assert.ok(steps.includes("node harness/ds-overflow/launch.mjs"));
});
