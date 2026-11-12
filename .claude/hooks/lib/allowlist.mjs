const SEGMENT = /^[^/*\\]+$/u;

const patternProblem = (pattern) => {
  const segments = pattern.split("/");
  const last = segments.length - 1;
  const bad = segments.find(
    (segment, index) =>
      !(
        SEGMENT.test(segment) ||
        segment === "*" ||
        (segment === "**" && index === last && index > 0)
      ) ||
      segment === "." ||
      segment === "..",
  );
  return bad === undefined ? undefined : `잘못된 조각 "${bad}"`;
};

export const parseAllowlist = (text) => {
  if (!text.endsWith("\n")) {
    return { problem: "마지막 줄이 줄바꿈으로 끝나지 않는다" };
  }
  const lines = text.slice(0, -1).split("\n");
  const includes = [];
  const excludes = [];
  const seen = new Set();
  for (const line of lines) {
    const negated = line.startsWith("!");
    const pattern = negated ? line.slice(1) : line;
    const problem =
      line.trim() !== line || pattern === ""
        ? "빈 줄이나 앞뒤 공백"
        : patternProblem(pattern);
    if (problem !== undefined) {
      return { problem: `${line}: ${problem}` };
    }
    if (seen.has(line)) {
      return { problem: `${line}: 같은 줄이 두 번 있다` };
    }
    seen.add(line);
    (negated ? excludes : includes).push(pattern);
  }
  return { includes, excludes };
};

export const patternMatches = (pattern, relative) => {
  const wanted = pattern.split("/");
  const actual = relative.split("/");
  const deep = wanted.at(-1) === "**";
  const fixed = deep ? wanted.slice(0, -1) : wanted;
  if (deep ? actual.length <= fixed.length : actual.length !== fixed.length) {
    return false;
  }
  return fixed.every(
    (segment, index) => segment === "*" || segment === actual[index],
  );
};

export const patternMatchesIgnoringCase = (pattern, relative) =>
  patternMatches(pattern.toLowerCase(), relative.toLowerCase());

export const isWritable = (allowlist, relative) =>
  allowlist.includes.some((pattern) => patternMatches(pattern, relative)) &&
  !allowlist.excludes.some((pattern) =>
    patternMatchesIgnoringCase(pattern, relative),
  );
