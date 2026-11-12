import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  isWritable,
  parseAllowlist,
  patternMatches,
} from "../../.claude/hooks/lib/allowlist.mjs";
import { globExpression } from "../../.claude/hooks/lib/sandbox-glob.mjs";

export { globExpression };

export const POLICY_FILE = "harness/sandbox/policy.json";
export const FRAGMENT_FILE = "harness/sandbox/lock-fragment.json";
export const ALLOWLIST_FILE = "harness/agent-writable.txt";

const segmentMatches = (wanted, actual) => wanted === "*" || wanted === actual;

const split = (pattern) => {
  const wanted = pattern.split("/");
  const deep = wanted.at(-1) === "**";
  return { fixed: deep ? wanted.slice(0, -1) : wanted, deep };
};

const reachesBelow = (pattern, directory) => {
  const { fixed, deep } = split(pattern);
  const limit = Math.min(fixed.length, directory.length);
  for (let index = 0; index < limit; index += 1) {
    if (!segmentMatches(fixed[index], directory[index])) {
      return false;
    }
  }
  return fixed.length > directory.length || deep;
};

const coversAllBelow = (pattern, directory) => {
  const { fixed, deep } = split(pattern);
  return (
    deep &&
    fixed.length <= directory.length &&
    fixed.every((segment, index) => segmentMatches(segment, directory[index]))
  );
};

const mayHoldWritable = (allowlist, keepWritable, directory) =>
  keepWritable.some((pattern) => reachesBelow(pattern, directory)) ||
  (allowlist.includes.some((pattern) => reachesBelow(pattern, directory)) &&
    !allowlist.excludes.some((pattern) =>
      coversAllBelow(
        pattern.toLowerCase(),
        directory.map((s) => s.toLowerCase()),
      ),
    ));

const childrenOf = (paths, prefix) => {
  const depth = prefix.length;
  const children = new Map();
  for (const path of paths) {
    if (prefix.every((segment, index) => path[index] === segment)) {
      const name = path[depth];
      const leaf = path.length === depth + 1;
      const known = children.get(name);
      children.set(name, known === undefined ? leaf : known && leaf);
    }
  }
  return [...children.entries()].sort(([left], [right]) =>
    left < right ? -1 : left > right ? 1 : 0,
  );
};

export const protectedDenies = (trackedPaths, allowlist, keepWritable) => {
  const paths = trackedPaths.map((path) => path.split("/"));
  const visit = (prefix) =>
    childrenOf(paths, prefix).flatMap(([name, leaf]) => {
      const here = [...prefix, name];
      const relative = here.join("/");
      if (leaf) {
        return isWritable(allowlist, relative) ? [] : [relative];
      }
      return mayHoldWritable(allowlist, keepWritable, here)
        ? visit(here)
        : [relative];
    });
  return visit([]);
};

export const lockFragment = (trackedPaths, allowlist, policy) => ({
  sandbox: {
    filesystem: {
      denyWrite: [
        ...new Set([
          ...protectedDenies(trackedPaths, allowlist, policy.lock.keepWritable),
          ...allowlist.excludes.map((pattern) =>
            pattern.endsWith("/**") ? pattern.slice(0, -3) : pattern,
          ),
          ...policy.lock.newPaths,
        ]),
      ].map((path) => `./${path}`),
    },
  },
});

export const trackedProtectedDenies = (trackedPaths, policy) =>
  trackedPaths
    .filter((path) =>
      policy.lock.trackedProtected.some((pattern) =>
        patternMatches(pattern, path),
      ),
    )
    .map((path) => `./${path}`);

export const lockedSandbox = (policy, fragment, trackedPaths = []) => ({
  ...policy.now,
  filesystem: {
    ...policy.now.filesystem,
    denyWrite: [
      ...policy.now.filesystem.denyWrite,
      ...fragment.sandbox.filesystem.denyWrite,
      ...trackedProtectedDenies(trackedPaths, policy),
    ],
  },
});

export const deniedBy = (denies, relative) =>
  denies
    .filter((entry) => entry.startsWith("./"))
    .some((entry) => globExpression(entry.slice(2)).test(relative));

export const trackedFiles = (root) =>
  execFileSync("git", ["-C", root, "ls-files", "-z"], { encoding: "utf8" })
    .split("\0")
    .filter((path) => path !== "");

export const readPolicy = (root) =>
  JSON.parse(readFileSync(join(root, POLICY_FILE), "utf8"));

export const readAllowlistFile = (root) =>
  parseAllowlist(readFileSync(join(root, ALLOWLIST_FILE), "utf8"));

export const expectedFragment = (root) =>
  lockFragment(trackedFiles(root), readAllowlistFile(root), readPolicy(root));

export const renderJson = (value) => `${JSON.stringify(value, undefined, 2)}\n`;

export { patternMatches };

const repoRoot = resolve(fileURLToPath(import.meta.url), "..", "..", "..");

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const expected = renderJson(expectedFragment(repoRoot));
  if (process.argv[2] === "--write") {
    writeFileSync(join(repoRoot, FRAGMENT_FILE), expected);
  }
  if (process.argv[2] === "--locked-settings") {
    const settings = JSON.parse(
      readFileSync(join(repoRoot, ".claude/settings.json"), "utf8"),
    );
    const fragment = JSON.parse(
      readFileSync(join(repoRoot, FRAGMENT_FILE), "utf8"),
    );
    process.stdout.write(
      renderJson({
        ...settings,
        sandbox: lockedSandbox(
          readPolicy(repoRoot),
          fragment,
          trackedFiles(repoRoot),
        ),
      }),
    );
  } else {
    const actual = readFileSync(join(repoRoot, FRAGMENT_FILE), "utf8");
    const same = actual === expected;
    process.stdout.write(
      same
        ? "sandbox lock fragment: 0 failure(s)\n"
        : `FAIL ${FRAGMENT_FILE}: node harness/sandbox/fragment.mjs --write 로 다시 만들어야 한다\nsandbox lock fragment: 1 failure(s)\n`,
    );
    process.exitCode = same ? 0 : 1;
  }
}
