export const VERDICT = Object.freeze({
  pass: "통과",
  fail: "실패",
  unsettled: "미확정",
});

export const ATTEMPT = Object.freeze({
  yes: "시도함",
  no: "시도 안 함",
});

const DENIAL =
  /[Oo]peration not permitted|PermissionError|denied|hook error|blocking error|\[[a-z-]+\]/u;

export const parseStream = (text) =>
  text
    .split("\n")
    .filter((line) => line.trim().startsWith("{"))
    .flatMap((line) => {
      try {
        return [JSON.parse(line)];
      } catch {
        return [];
      }
    });

const contentOf = (event) =>
  Array.isArray(event.message?.content) ? event.message.content : [];

const flatText = (value) =>
  typeof value === "string"
    ? value
    : Array.isArray(value)
      ? value.map(flatText).join("\n")
      : typeof value === "object" && value !== null
        ? Object.values(value).map(flatText).join("\n")
        : "";

const parentOf = (event) => event.parent_tool_use_id ?? null;

export const toolUses = (events) =>
  events.flatMap((event, index) =>
    event.type === "assistant"
      ? contentOf(event)
          .filter((part) => part.type === "tool_use")
          .map((part) => ({
            id: part.id,
            name: part.name,
            input: part.input ?? {},
            parent: parentOf(event),
            index,
          }))
      : [],
  );

const PROGRESS = new Set(["task_started", "task_progress", "task_updated"]);

const windowText = (events, use) =>
  events
    .slice(use.index + 1)
    .flatMap((event) => {
      const results = contentOf(event)
        .filter(
          (part) => part.type === "tool_result" && part.tool_use_id === use.id,
        )
        .map((part) => flatText(part.content));
      const mentions =
        event.type === "system" &&
        !PROGRESS.has(event.subtype) &&
        event.tool_use_id === use.id
          ? [flatText(event)]
          : [];
      return [...results, ...mentions];
    })
    .join("\n");

const matchingUses = (events, probe) =>
  toolUses(events).filter(
    (use) =>
      new RegExp(probe.tool, "u").test(use.name) &&
      new RegExp(probe.input, "u").test(JSON.stringify(use.input)) &&
      (probe.agent === "subagent" ? use.parent !== null : use.parent === null),
  );

const shortened = (text) => {
  const line = text
    .split("\n")
    .map((part) => part.trim())
    .find((part) => part !== "");
  const single = (line ?? "").replaceAll("|", "/");
  return single.length > 90 ? `${single.slice(0, 87)}...` : single;
};

const evidenceLine = (text, pattern) => {
  const found = text
    .split("\n")
    .find((line) => new RegExp(pattern, "u").test(line));
  return shortened(found ?? text);
};

const judgeTool = (experiment, observed) => {
  const session = observed.sessions[experiment.probe.session];
  const uses = matchingUses(session?.events ?? [], experiment.probe);
  const inDebug = experiment.probe.kind === "tool-debug";
  if (uses.length === 0) {
    return {
      attempt: ATTEMPT.no,
      verdict: VERDICT.unsettled,
      actual: "모델이 시도하지 않았다",
    };
  }
  const texts = inDebug
    ? [session.debug ?? ""]
    : uses.map((use) => windowText(session.events, use));
  const evidence = new RegExp(experiment.evidence, "u");
  const fits = (text) =>
    experiment.expect === "denied"
      ? evidence.test(text)
      : evidence.test(text) && !DENIAL.test(text);
  const fitting = texts.find(fits);
  return {
    attempt: ATTEMPT.yes,
    verdict: fitting === undefined ? VERDICT.fail : VERDICT.pass,
    actual:
      fitting === undefined
        ? shortened(texts[0])
        : evidenceLine(fitting, experiment.evidence),
  };
};

const judgeToolList = (experiment, observed) => {
  const init = (observed.sessions[experiment.probe.session]?.events ?? []).find(
    (event) => event.type === "system" && event.subtype === "init",
  );
  if (init === undefined) {
    return {
      attempt: ATTEMPT.no,
      verdict: VERDICT.unsettled,
      actual: "세션 시작 기록이 없다",
    };
  }
  const present = (init.tools ?? []).includes(experiment.probe.absent);
  return {
    attempt: ATTEMPT.yes,
    verdict: present ? VERDICT.fail : VERDICT.pass,
    actual: present
      ? `${experiment.probe.absent}가 도구 목록에 있다`
      : `${experiment.probe.absent}가 도구 목록에 없다`,
  };
};

const judgeDebug = (experiment, observed) => {
  const session = observed.sessions[experiment.probe.session];
  if (session?.markers?.[experiment.probe.marker] !== true) {
    return {
      attempt: ATTEMPT.no,
      verdict: VERDICT.unsettled,
      actual: "변경을 넣을 시점(모델의 표시 명령)이 오지 않았다",
    };
  }
  const debug = session.debug ?? "";
  const evidence = new RegExp(experiment.evidence, "u");
  return {
    attempt: ATTEMPT.yes,
    verdict: evidence.test(debug) ? VERDICT.pass : VERDICT.fail,
    actual: evidence.test(debug)
      ? evidenceLine(debug, experiment.evidence)
      : "디버그 기록에 차단이 없다",
  };
};

const recordFits = (record, expected) =>
  Object.entries(expected).every(([key, value]) =>
    key === "minBlocks" ? (record.blocks ?? 0) >= value : record[key] === value,
  );

const judgeRecord = (experiment, observed) => {
  const session = observed.sessions[experiment.probe.session];
  const uses = matchingUses(session?.events ?? [], experiment.probe);
  const records = observed.records.filter(
    (record) => record.kind === experiment.probe.recordKind,
  );
  if (uses.length === 0 || records.length === 0) {
    return {
      attempt: ATTEMPT.no,
      verdict: VERDICT.unsettled,
      actual:
        uses.length === 0
          ? "모델이 시도하지 않았다"
          : `${experiment.probe.recordKind} 기록이 없다`,
    };
  }
  const fitting = records.find((record) =>
    recordFits(record.data, experiment.record),
  );
  const shown = fitting ?? records[0];
  return {
    attempt: ATTEMPT.yes,
    verdict: fitting === undefined ? VERDICT.fail : VERDICT.pass,
    actual: `상태=${shown.data.status ?? "없음"}, 차단=${shown.data.blocks ?? 0}`,
  };
};

const JUDGES = {
  tool: judgeTool,
  "tool-debug": judgeTool,
  "tool-list": judgeToolList,
  debug: judgeDebug,
  record: judgeRecord,
};

export const judge = (experiment, observed) => {
  const decide = JUDGES[experiment.probe.kind];
  const outcome =
    decide === undefined
      ? {
          attempt: ATTEMPT.no,
          verdict: VERDICT.fail,
          actual: `모르는 실험 종류 ${experiment.probe.kind}`,
        }
      : decide(experiment, observed);
  return {
    id: experiment.id,
    source: experiment.source,
    title: experiment.title,
    expected: experiment.expected,
    ...outcome,
  };
};

export const judgeAll = (experiments, observed) =>
  experiments.map((experiment) => judge(experiment, observed));

export const overall = (rows) => {
  const failed = rows.filter((row) => row.verdict === VERDICT.fail).length;
  const unsettled = rows.filter(
    (row) => row.verdict === VERDICT.unsettled,
  ).length;
  return {
    passed: failed === 0 && unsettled === 0,
    failed,
    unsettled,
  };
};

export const resultTable = (rows, version) => {
  const summary = overall(rows);
  return [
    `Claude Code ${version}`,
    "",
    "| 실험 | 출처 | 내용 | 기대 | 실제 | 모델의 시도 | 판정 |",
    "|---|---|---|---|---|---|---|",
    ...rows.map(
      (row) =>
        `| ${row.id} | ${row.source} | ${row.title} | ${row.expected} | ${row.actual} | ${row.attempt} | ${row.verdict} |`,
    ),
    "",
    summary.passed
      ? `claude-regression: 통과 ${rows.length}건`
      : `claude-regression: 실패 (실패 ${summary.failed}건, 미확정 ${summary.unsettled}건)${summary.unsettled > 0 ? ". 미확정은 모델이 시도하지 않은 실험이다. just claude-regression을 다시 돌린다" : ""}`,
  ].join("\n");
};

const RECORD_FIELDS = {
  implementer: ["blocks", "status"],
  reviewer: ["minBlocks"],
};

const PROBE_FIELDS = {
  tool: ["agent", "input", "kind", "session", "step", "tool"],
  "tool-debug": ["agent", "input", "kind", "session", "step", "tool"],
  "tool-list": ["absent", "kind", "session"],
  debug: ["kind", "marker", "session"],
  record: ["agent", "input", "kind", "recordKind", "session", "step", "tool"],
};

const EXPERIMENT_FIELDS = {
  tool: ["evidence", "expect", "expected", "id", "probe", "source", "title"],
  "tool-debug": [
    "evidence",
    "expect",
    "expected",
    "id",
    "probe",
    "source",
    "title",
  ],
  "tool-list": ["expected", "id", "probe", "source", "title"],
  debug: ["evidence", "expected", "id", "probe", "source", "title"],
  record: ["expected", "id", "probe", "record", "source", "title"],
};

const sameKeys = (object, keys) =>
  typeof object === "object" &&
  object !== null &&
  JSON.stringify(Object.keys(object).sort()) === JSON.stringify(keys);

const r10Numbers = (source) =>
  [...source.matchAll(/#(\d+)(?:~#(\d+))?/gu)].flatMap((match) => {
    const first = Number.parseInt(match[1], 10);
    const last = match[2] === undefined ? first : Number.parseInt(match[2], 10);
    return Array.from(
      { length: last - first + 1 },
      (_, offset) => first + offset,
    );
  });

export const coveredR10 = (experiments) =>
  [
    ...new Set(
      experiments.flatMap((experiment) =>
        String(experiment.source ?? "")
          .split(",")
          .map((part) => part.trim())
          .filter((part) => part.startsWith("R-10"))
          .flatMap(r10Numbers),
      ),
    ),
  ].sort((a, b) => a - b);

const experimentProblems = (experiment, sessions) => {
  const kind = experiment.probe?.kind;
  if (!(kind in EXPERIMENT_FIELDS)) {
    return [`${experiment.id}: 모르는 실험 종류`];
  }
  const session = sessions.find((item) => item.id === experiment.probe.session);
  const prompt = session?.prompt ?? [];
  return [
    ...(sameKeys(experiment, EXPERIMENT_FIELDS[kind])
      ? []
      : [
          `${experiment.id}: 실험 칸이 ${EXPERIMENT_FIELDS[kind].join(", ")}가 아니다`,
        ]),
    ...(sameKeys(experiment.probe, PROBE_FIELDS[kind])
      ? []
      : [
          `${experiment.id}: probe 칸이 ${PROBE_FIELDS[kind].join(", ")}가 아니다`,
        ]),
    ...(session === undefined ? [`${experiment.id}: 세션이 없다`] : []),
    ...("step" in experiment.probe &&
    !(prompt[experiment.probe.step] ?? "").startsWith(
      `${experiment.probe.step}. `,
    )
      ? [`${experiment.id}: 세션 지시에 ${experiment.probe.step}단계가 없다`]
      : []),
    ...(kind === "debug" && session?.configMarker === undefined
      ? [`${experiment.id}: 세션에 변경 시점 표시가 없다`]
      : []),
    ...(kind === "record" &&
    !sameKeys(
      experiment.record,
      RECORD_FIELDS[experiment.probe.recordKind] ?? [],
    )
      ? [`${experiment.id}: record 칸이 기록 종류와 맞지 않는다`]
      : []),
  ];
};

export const specProblems = (spec) => {
  if (!sameKeys(spec, ["experiments", "sessions"])) {
    return ["experiments.json: 최상위는 sessions와 experiments다"];
  }
  const sessions = spec.sessions;
  const sessionProblems = sessions.flatMap((session) => [
    ...(typeof session.id === "string" && Array.isArray(session.prompt)
      ? []
      : ["세션에 id와 prompt가 있어야 한다"]),
    ...(session.prompt ?? [])
      .slice(1)
      .filter((line, index) => !line.startsWith(`${index + 1}. `))
      .map(
        (line) =>
          `${session.id}: 단계 번호가 이어지지 않는다: ${line.slice(0, 40)}`,
      ),
    ...(session.prompt ?? [])
      .slice(1)
      .map((line, index) => index + 1)
      .filter(
        (step) =>
          !spec.experiments.some(
            (experiment) =>
              experiment.probe?.session === session.id &&
              experiment.probe?.step === step,
          ) &&
          !(
            session.configMarker !== undefined &&
            (session.prompt?.[step] ?? "").includes(session.configMarker)
          ),
      )
      .map((step) => `${session.id}: ${step}단계를 보는 실험이 없다`),
    ...(session.configMarker !== undefined &&
    !(session.prompt ?? []).some((line) => line.includes(session.configMarker))
      ? [
          `${session.id}: 변경 시점 표시 ${session.configMarker}를 시키는 단계가 없다`,
        ]
      : []),
  ]);
  const ids = spec.experiments.map((experiment) => experiment.id);
  const duplicates = ids
    .filter((id, index) => ids.indexOf(id) !== index)
    .map((id) => `실험 ID ${id}가 두 번 있다`);
  const missingR10 = Array.from({ length: 12 }, (_, index) => index + 1)
    .filter((number) => !coveredR10(spec.experiments).includes(number))
    .map((number) => `R-10 실험 #${number}를 보는 실험이 없다`);
  return [
    ...sessionProblems,
    ...duplicates,
    ...missingR10,
    ...spec.experiments.flatMap((experiment) =>
      experimentProblems(experiment, sessions),
    ),
  ];
};
