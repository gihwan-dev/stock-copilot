import { strict as assert } from "node:assert";
import { test } from "node:test";
import { execFileSync, spawnSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdirSync,
  renameSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { decideBash } from "../../../.claude/hooks/lib/bash-guard.mjs";
import { createFixture, projectFor, repoRoot } from "./fixture.mjs";

const remove =
  process.platform === "darwin" ? "rm -xrf" : "rm -rf --one-file-system";

const verdict = (fixture, role, command) => {
  const project = {
    ...projectFor(fixture, { lock: "on" }),
    ownedWorktree: fixture.worktree,
  };
  return decideBash(
    {
      cwd: fixture.worktree,
      agent_type: role,
      agent_id: "review",
      tool_input: { command },
    },
    project,
  ).decision;
};

test("공통 역할 가드는 지정된 임시 디렉터리 정리만 허용한다", () => {
  const fixture = createFixture();
  try {
    mkdirSync(join(fixture.worktree, ".rv-owned"));
    mkdirSync(join(fixture.worktree, "target"));
    symlinkSync(fixture.outside, join(fixture.worktree, ".rv-link"));
    for (const role of ["implementer", "unknown"])
      assert.equal(verdict(fixture, role, `${remove} .rv-owned`), "deny", role);
    for (const role of ["adversarial-reviewer", "review-critic"]) {
      for (const command of [`${remove} .rv-owned`, `${remove} target`])
        assert.equal(
          verdict(fixture, role, command),
          "allow",
          `${role} ${command}`,
        );
      for (const command of [
        `${remove} docs/notes.md`,
        `${remove} ~`,
        `${remove} .rv-owned/.`,
        `${remove} -P .rv-owned`,
        `sh -c '${remove} .rv-link'`,
        `${remove} ${fixture.outside}`,
        `${remove} ${fixture.root}/target`,
        `${remove} .rv-link`,
        `${remove} target/../docs`,
        `${remove} target/`,
        `${remove} .rv-owned/`,
        `${remove} .rv-owned docs/notes.md`,
        `${remove} .rv-owned --`,
        `${remove} .rv-owned -f`,
        "rm -rf .rv-owned",
        process.platform === "darwin"
          ? "rm -rf .rv-owned -x"
          : "rm -rf .rv-owned --one-file-system",
        "rm -rf -- .rv-owned -x",
        "touch .rv-owned/new.txt",
        `${remove} target > .rv-owned/new.txt`,
      ])
        assert.equal(
          verdict(fixture, role, command),
          "deny",
          `${role} ${command}`,
        );
      for (const command of ["rm -rf target", "rm -rf .rv-owned"]) {
        const project = {
          ...projectFor(fixture, { lock: "on" }),
          ownedWorktree: fixture.worktree,
        };
        const denied = decideBash(
          {
            cwd: fixture.worktree,
            agent_type: role,
            agent_id: "review",
            tool_input: { command },
          },
          project,
        );
        assert.equal(denied.decision, "deny", `${role} ${command}`);
        assert.equal(denied.findings[0].rule, "reviewer-cleanup");
        assert.match(denied.findings[0].message, new RegExp(remove, "u"));
      }
    }
    rmSync(join(fixture.worktree, "target"), { recursive: true });
    symlinkSync(fixture.outside, join(fixture.worktree, "target"));
    for (const role of ["adversarial-reviewer", "review-critic"])
      for (const target of ["target", "target/"])
        assert.equal(
          verdict(fixture, role, `${remove} ${target}`),
          "deny",
          "target 심볼릭 링크",
        );
    writeFileSync(join(fixture.worktree, ".rv-owned", "scratch"), "scratch\n");
    execFileSync(
      "rm",
      process.platform === "darwin"
        ? ["-xrf", ".rv-owned"]
        : ["-rf", "--one-file-system", ".rv-owned"],
      { cwd: fixture.worktree },
    );
    assert.equal(existsSync(join(fixture.worktree, ".rv-owned")), false);
    const nestedWorktree = join(fixture.worktree, ".rv-linked");
    const cleanEnv = Object.fromEntries(
      Object.entries(process.env).filter(([name]) => !name.startsWith("GIT_")),
    );
    execFileSync(
      "git",
      [
        "-C",
        fixture.worktree,
        "worktree",
        "add",
        "--detach",
        "--quiet",
        nestedWorktree,
      ],
      { env: cleanEnv },
    );
    for (const role of ["adversarial-reviewer", "review-critic"])
      assert.equal(
        verdict(fixture, role, `${remove} .rv-linked`),
        "deny",
        "다른 등록 작업 사본",
      );
    const caseRoot = join(fixture.worktree, ".rv-case");
    const caseAlias = join(fixture.worktree, ".RV-CASE");
    mkdirSync(caseRoot);
    if (!existsSync(caseAlias)) symlinkSync(".rv-case", caseAlias);
    execFileSync(
      "git",
      [
        "-C",
        fixture.worktree,
        "worktree",
        "add",
        "--detach",
        "--quiet",
        join(caseAlias, "nested"),
      ],
      { env: cleanEnv },
    );
    for (const role of ["adversarial-reviewer", "review-critic"])
      assert.equal(
        verdict(fixture, role, `${remove} .rv-case`),
        "deny",
        "다른 등록 작업 사본의 대소문자·링크 별칭",
      );
    mkdirSync(join(fixture.worktree, ".rv-tracked"));
    writeFileSync(
      join(fixture.worktree, ".rv-tracked", "tracked.txt"),
      "keep\n",
    );
    execFileSync(
      "git",
      ["-C", fixture.worktree, "add", ".rv-tracked/tracked.txt"],
      {
        env: Object.fromEntries(
          Object.entries(process.env).filter(
            ([name]) => !name.startsWith("GIT_"),
          ),
        ),
      },
    );
    for (const role of ["adversarial-reviewer", "review-critic"])
      assert.equal(verdict(fixture, role, `${remove} .rv-tracked`), "deny");
    mkdirSync(join(fixture.worktree, ".rv-unregistered"));
    const unregistered = {
      ...projectFor(fixture, { lock: "on" }),
      ownedWorktree: fixture.worktree,
      worktrees: [fixture.root],
    };
    const unregisteredResult = decideBash(
      {
        cwd: fixture.worktree,
        agent_type: "adversarial-reviewer",
        agent_id: "review",
        tool_input: { command: `${remove} .rv-unregistered` },
      },
      unregistered,
    );
    assert.equal(
      unregisteredResult.decision,
      "deny",
      "미등록 작업 사본 정리 거부",
    );
    mkdirSync(join(fixture.root, ".rv-main"));
    const mainProject = {
      ...projectFor(fixture, { lock: "on" }),
      ownedWorktree: fixture.root,
    };
    const mainResult = decideBash(
      {
        cwd: fixture.root,
        agent_type: "adversarial-reviewer",
        agent_id: "review",
        tool_input: { command: `${remove} .rv-main` },
      },
      mainProject,
    );
    assert.equal(mainResult.decision, "deny", "메인 저장소 정리 거부");
    const gitEnv = {
      ...Object.fromEntries(
        Object.entries(process.env).filter(
          ([name]) => !name.startsWith("GIT_"),
        ),
      ),
      GIT_CONFIG_GLOBAL: "/dev/null",
      GIT_CONFIG_NOSYSTEM: "1",
      GIT_AUTHOR_NAME: "Fixture",
      GIT_AUTHOR_EMAIL: "fixture@example.invalid",
      GIT_COMMITTER_NAME: "Fixture",
      GIT_COMMITTER_EMAIL: "fixture@example.invalid",
    };
    execFileSync(
      "git",
      ["-C", fixture.worktree, "commit", "--quiet", "-m", "tracked"],
      { env: gitEnv },
    );
    execFileSync(
      "git",
      [
        "-C",
        fixture.worktree,
        "rm",
        "--quiet",
        "--cached",
        ".rv-tracked/tracked.txt",
      ],
      { env: gitEnv },
    );
    for (const role of ["adversarial-reviewer", "review-critic"])
      assert.equal(
        verdict(fixture, role, `${remove} .rv-tracked`),
        "deny",
        "HEAD 추적 파일의 스테이징 삭제",
      );
    for (const [trackedDir, lowerDir] of [
      [".RV-UPPER", ".rv-upper"],
      ["TARGET", "target"],
    ]) {
      const upperPath = join(fixture.worktree, trackedDir);
      const lowerPath = join(fixture.worktree, lowerDir);
      if (existsSync(lowerPath))
        rmSync(lowerPath, { recursive: true, force: true });
      mkdirSync(upperPath);
      writeFileSync(join(upperPath, "secret.txt"), "keep\n");
      execFileSync(
        "git",
        ["-C", fixture.worktree, "add", `${trackedDir}/secret.txt`],
        { env: gitEnv },
      );
      execFileSync(
        "git",
        [
          "-C",
          fixture.worktree,
          "commit",
          "--quiet",
          "-m",
          `track ${trackedDir}`,
        ],
        { env: gitEnv },
      );
      const tempSwap = join(fixture.worktree, ".rv-swap-temp");
      renameSync(upperPath, tempSwap);
      renameSync(tempSwap, lowerPath);
      for (const role of ["adversarial-reviewer", "review-critic"])
        assert.equal(
          verdict(fixture, role, `${remove} ${lowerDir}`),
          "deny",
          `${trackedDir} 대소문자 별칭 ${lowerDir} 거부`,
        );
    }
  } finally {
    rmSync(fixture.base, { recursive: true, force: true });
  }
});

test("실제 Claude 진입 훅은 소유한 작업 사본에서만 정리를 허용한다", () => {
  const fixture = createFixture();
  try {
    cpSync(
      join(repoRoot, ".claude/hooks"),
      join(fixture.worktree, ".claude/hooks"),
      { recursive: true },
    );
    cpSync(
      join(repoRoot, "harness/claude-hooks/rules.json"),
      join(fixture.worktree, "harness/claude-hooks/rules.json"),
    );
    cpSync(
      join(repoRoot, "harness/agent-writable.txt"),
      join(fixture.worktree, "harness/agent-writable.txt"),
    );
    writeFileSync(
      join(fixture.worktree, "harness/claude-hooks/lock.txt"),
      "on\n",
    );
    mkdirSync(join(fixture.worktree, ".rv-owned"));
    const invoke = (command) =>
      spawnSync(
        process.execPath,
        [join(fixture.worktree, ".claude/hooks/guard-bash.mjs")],
        {
          cwd: fixture.worktree,
          env: { ...process.env, CLAUDE_PROJECT_DIR: fixture.worktree },
          input: JSON.stringify({
            cwd: fixture.worktree,
            tool_input: { command },
          }),
          encoding: "utf8",
        },
      );
    const allowed = invoke(`${remove} .rv-owned`);
    assert.equal(allowed.status, 0, allowed.stderr);
    assert.equal(allowed.stdout, "");
    const denied = invoke(`${remove} docs/notes.md`);
    assert.equal(denied.status, 0, denied.stderr);
    assert.equal(
      JSON.parse(denied.stdout).hookSpecificOutput.permissionDecision,
      "deny",
    );
  } finally {
    rmSync(fixture.base, { recursive: true, force: true });
  }
});

test("리눅스에서는 다른 파일시스템을 건너뛰는 옵션을 요구한다", () => {
  const fixture = createFixture();
  const platform = Object.getOwnPropertyDescriptor(process, "platform");
  try {
    mkdirSync(join(fixture.worktree, ".rv-owned"));
    Object.defineProperty(process, "platform", { value: "linux" });
    assert.equal(
      verdict(
        fixture,
        "adversarial-reviewer",
        "rm -rf --one-file-system .rv-owned",
      ),
      "allow",
    );
    for (const command of [
      "rm -rf .rv-owned",
      "rm -xrf .rv-owned",
      "rm -rf .rv-owned --one-file-system",
      "rm -rf -- .rv-owned --one-file-system",
    ])
      assert.equal(verdict(fixture, "adversarial-reviewer", command), "deny");
  } finally {
    Object.defineProperty(process, "platform", platform);
    rmSync(fixture.base, { recursive: true, force: true });
  }
});
