import { createHash } from "node:crypto";

const BASE_LINE =
  /^[ \t]*(?:[-*][ \t]*)?기준 커밋[ \t]*:[ \t]*`?([^`\s,]*)`?(.*)$/gmu;
const BRANCH_LINE =
  /^[ \t]*(?:[-*][ \t]*)?브랜치[ \t]*:[ \t]*`?([^`\s,]*)`?[ \t]*$/gmu;
const BRANCH_INLINE = /브랜치[ \t]*`([^`\s]*)`/u;
const COMMIT_TEXT = /^[0-9a-f]{7,40}$/u;
const BRANCH_NAME = /^(?!-)(?!.*\.\.)[A-Za-z0-9._/-]{1,100}$/u;

export const BRIEF_PROBLEM = Object.freeze({
  noBase: "no-base",
  manyBases: "many-bases",
  badBase: "bad-base",
  manyBranches: "many-branches",
  badBranch: "bad-branch",
});

const distinct = (values) => [...new Set(values)];

export const parseBrief = (prompt) => {
  const text = typeof prompt === "string" ? prompt : "";
  const bases = [...text.matchAll(BASE_LINE)];
  if (bases.length === 0) {
    return { problem: BRIEF_PROBLEM.noBase };
  }
  const commits = distinct(bases.map((match) => match[1]));
  if (commits.length > 1) {
    return { problem: BRIEF_PROBLEM.manyBases, values: commits };
  }
  const [commit] = commits;
  if (!COMMIT_TEXT.test(commit)) {
    return { problem: BRIEF_PROBLEM.badBase, values: [commit] };
  }
  const branches = distinct([
    ...bases
      .map((match) => BRANCH_INLINE.exec(match[2]))
      .filter((match) => match !== null)
      .map((match) => match[1]),
    ...[...text.matchAll(BRANCH_LINE)].map((match) => match[1]),
  ]);
  if (branches.length > 1) {
    return { problem: BRIEF_PROBLEM.manyBranches, values: branches };
  }
  const branch = branches[0];
  if (branch !== undefined && !BRANCH_NAME.test(branch)) {
    return { problem: BRIEF_PROBLEM.badBranch, values: [branch] };
  }
  return { commit, branch };
};

export const digestOf = (text) =>
  createHash("sha256")
    .update(typeof text === "string" ? text : "")
    .digest("hex");
