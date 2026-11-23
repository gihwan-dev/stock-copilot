const kinds = new Set([
  "document-scroll",
  "unauthorized-overflow",
  "clipped",
  "missing-tooltip",
  "nested-scroll",
  "outside-window",
  "overlapping-controls",
]);

const text = (value) => typeof value === "string" && value.trim().length > 0;
const conditionKey = (row) => JSON.stringify([row.id, row.width, row.scale]);
const isFinding = (value) =>
  value !== null &&
  typeof value === "object" &&
  Object.keys(value).sort().join(",") === "element,kind" &&
  kinds.has(value.kind) &&
  text(value.element);
const validCondition = (row, pending) =>
  row !== null &&
  typeof row === "object" &&
  text(row.story) &&
  text(row.id) &&
  ["normal", "narrow"].includes(row.width) &&
  ["100%", "125%"].includes(row.scale) &&
  Array.isArray(row.findings) &&
  row.findings.length > 0 &&
  row.findings.every(isFinding) &&
  (!pending || text(row.reason));

const canonical = (row) =>
  JSON.stringify({
    story: row.story,
    id: row.id,
    width: row.width,
    scale: row.scale,
    findings: row.findings
      .map((finding) => JSON.stringify([finding.kind, finding.element]))
      .sort(),
  });

const indexed = (rows, pending, problems) => {
  const result = new Map();
  for (const row of rows) {
    if (!validCondition(row, pending)) {
      problems.push(`${pending ? "보류" : "측정"} 조건이 올바르지 않다`);
      continue;
    }
    const key = conditionKey(row);
    if (result.has(key)) {
      problems.push(`${pending ? "보류" : "측정"} 조건 중복: ${key}`);
      continue;
    }
    result.set(key, canonical(row));
  }
  return result;
};

export const comparePending = (actual, expected) => {
  if (!Array.isArray(actual) || !Array.isArray(expected))
    return ["측정과 보류는 각각 배열이어야 한다"];
  const problems = [];
  const measured = indexed(actual, false, problems);
  const pending = indexed(expected, true, problems);
  for (const [key, value] of measured) {
    if (pending.get(key) !== value) problems.push(`새롭거나 다른 판정: ${key}`);
  }
  for (const [key, value] of pending) {
    if (measured.get(key) !== value)
      problems.push(`사라졌거나 다른 보류: ${key}`);
  }
  return problems;
};
