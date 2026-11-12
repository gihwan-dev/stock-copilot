import { join } from "node:path";

const ISOLATION = ".claude/worktrees";

export const watchedWorktrees = (worktrees, projectRoot) =>
  worktrees.filter(
    (path) => !path.startsWith(`${join(projectRoot, ISOLATION)}/`),
  );

const relevant = (path) => !path.startsWith(`${ISOLATION}/`);

export const snapshotOf = (worktrees, projectRoot, facts) =>
  watchedWorktrees(worktrees, projectRoot)
    .sort()
    .map((path) => {
      const taken = facts.contentSnapshot(path);
      return taken.failed === undefined
        ? {
            path,
            head: taken.head,
            index: taken.index,
            files: Object.fromEntries(
              Object.entries(taken.files).filter(([file]) => relevant(file)),
            ),
          }
        : { path, failed: taken.failed };
    });

const fileDifferences = (path, before, after) => {
  const names = [
    ...new Set([...Object.keys(before), ...Object.keys(after)]),
  ].sort();
  return names
    .filter((name) => before[name] !== after[name])
    .map((name) =>
      before[name] === undefined
        ? `${path}: 새 파일 ${name}`
        : after[name] === undefined
          ? `${path}: 사라진 파일 ${name}`
          : `${path}: 내용이 바뀐 파일 ${name}`,
    );
};

export const snapshotDifferences = (before, after) => {
  const earlier = new Map(before.map((entry) => [entry.path, entry]));
  const later = new Map(after.map((entry) => [entry.path, entry]));
  return [
    ...before
      .filter((entry) => !later.has(entry.path))
      .map((entry) => `${entry.path}: 작업 사본이 사라졌다`),
    ...after.flatMap((entry) => {
      const old = earlier.get(entry.path);
      if (old === undefined) {
        return [`${entry.path}: 새 작업 사본이 생겼다`];
      }
      if (old.failed !== undefined || entry.failed !== undefined) {
        return [
          `${entry.path}: 상태를 읽지 못했다 (${entry.failed ?? old.failed})`,
        ];
      }
      return [
        ...(old.head === entry.head
          ? []
          : [`${entry.path}: HEAD ${old.head} -> ${entry.head}`]),
        ...(old.index === entry.index
          ? []
          : [`${entry.path}: 인덱스가 바뀌었다`]),
        ...fileDifferences(entry.path, old.files, entry.files),
      ];
    }),
  ];
};

export const decideReviewerStop = (input, record, snapshot, rules, now) => {
  if (record === undefined) {
    return {
      record: {
        kind: "reviewer",
        agentId: input.agent_id,
        agentType: input.agent_type,
        status: "unchecked",
        reason: "시작 스냅숏이 없다",
        updatedAt: now,
      },
      output: {
        systemMessage: `${input.agent_type} ${input.agent_id}: 시작 스냅숏이 없어 원본 변경을 확인하지 못했다`,
      },
    };
  }
  const differences = snapshotDifferences(record.snapshot, snapshot);
  if (differences.length === 0) {
    return {
      record: { ...record, status: "clean", updatedAt: now },
      output: undefined,
    };
  }
  const blocks = (record.blocks ?? 0) + 1;
  if (blocks > rules.maxBlocks) {
    return {
      record: {
        ...record,
        blocks,
        status: "contaminated",
        differences,
        updatedAt: now,
      },
      output: {
        systemMessage: `${input.agent_type} ${input.agent_id}가 원본 저장소나 작업 사본을 바꾼 채 끝났다. 메인은 다음 단계로 가지 말고 사용자에게 보고한다:\n${differences.join("\n")}`,
      },
    };
  }
  return {
    record: {
      ...record,
      blocks,
      status: "contaminated",
      differences,
      updatedAt: now,
    },
    output: {
      decision: "block",
      reason: `리뷰어·비판자는 원본 저장소와 작업 사본에 쓰지 않는다 (docs/harness.md 7.5). 네가 만든 변경을 되돌린 뒤 끝내라 (${blocks}/${rules.maxBlocks}):\n${differences.join("\n")}`,
    },
  };
};
