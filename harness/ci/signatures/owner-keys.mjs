import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";

export const ownerKeysUrl = (owner) =>
  `https://api.github.com/users/${encodeURIComponent(owner)}/ssh_signing_keys`;

export const ownerKeysFromResponse = (text) => {
  try {
    const parsed = JSON.parse(text);
    return Array.isArray(parsed)
      ? {
          keys: parsed
            .map((entry) =>
              typeof entry?.key === "string"
                ? entry.key.trim().split(/\s+/u).slice(0, 2).join(" ")
                : "",
            )
            .filter((key) => key !== ""),
        }
      : { problem: "소유자 키 응답이 배열이 아니다" };
  } catch {
    return { problem: "소유자 키 응답을 읽지 못했다" };
  }
};

export const ownerKeysRequest = (owner, token) => [
  "curl",
  [
    "-sSfL",
    "--max-time",
    "30",
    "-H",
    "Accept: application/vnd.github+json",
    ...(token === "" ? [] : ["-H", `Authorization: Bearer ${token}`]),
    ownerKeysUrl(owner),
  ],
];

export const fetchedOwnerKeys = (owner, token) => () => {
  if (!/^[A-Za-z0-9-]+$/u.test(owner)) {
    return {
      problem:
        "저장소 소유자 이름(harness/ci/signatures/owner.txt)이 없거나 잘못됐다",
    };
  }
  const [program, args] = ownerKeysRequest(owner, token);
  const finished = spawnSync(program, args, { encoding: "utf8" });
  return finished.status === 0
    ? ownerKeysFromResponse(finished.stdout)
    : { problem: `소유자 키를 가져오지 못했다: ${finished.stderr.trim()}` };
};

export const fileOwnerKeys = (file) => () => ({
  keys: readFileSync(file, "utf8")
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line !== ""),
});

export const missingOwnerKeys = () => ({
  problem:
    "저장소 소유자의 SSH 서명 키 목록이 없어 판정할 수 없다(CI만 판정한다)",
});
