export const VERDICT = Object.freeze({
  pass: "통과",
  fail: "실패",
  unsettled: "미확정",
});

export const ATTEMPT = Object.freeze({
  yes: "시도함",
  no: "시도 안 함",
});

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

export const fill = (text, values) =>
  Object.entries(values ?? {}).reduce(
    (current, [key, value]) => current.replaceAll(`{${key}}`, value),
    text,
  );

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

const resultOf = (events, use) => {
  const event = events
    .slice(use.index + 1)
    .find((candidate) =>
      contentOf(candidate).some(
        (part) => part.type === "tool_result" && part.tool_use_id === use.id,
      ),
    );
  const part = contentOf(event ?? {}).find(
    (candidate) =>
      candidate.type === "tool_result" && candidate.tool_use_id === use.id,
  );
  return part === undefined
    ? undefined
    : {
        error: part.is_error === true,
        text: flatText(part.content),
        timestamp: event.timestamp,
      };
};

const matchingUses = (events, probe, pattern = probe.input) =>
  toolUses(events).filter(
    (use) =>
      new RegExp(probe.tool, "u").test(use.name) &&
      new RegExp(pattern, "u").test(JSON.stringify(use.input)) &&
      (probe.agent === "subagent" ? use.parent !== null : use.parent === null),
  );

const shortened = (text) => {
  const line = (text ?? "")
    .split("\n")
    .map((part) => part.trim())
    .find((part) => part !== "");
  const single = (line ?? "").replaceAll("|", "/");
  return single.length > 90 ? `${single.slice(0, 87)}...` : single;
};

const unsettled = (actual) => ({
  attempt: ATTEMPT.no,
  verdict: VERDICT.unsettled,
  actual,
});

const decided = (passed, actual) => ({
  attempt: ATTEMPT.yes,
  verdict: passed ? VERDICT.pass : VERDICT.fail,
  actual,
});

const changedTargets = (experiment, observed) =>
  (experiment.unchanged ?? [])
    .map((target) => fill(target, observed.values))
    .filter(
      (target) =>
        observed.snapshots?.before?.[target] === undefined ||
        observed.snapshots.before[target] !==
          observed.snapshots?.after?.[target],
    );

const hookDenied = (use, result, rule, debug) =>
  result !== undefined &&
  result.error &&
  result.text.startsWith(`PreToolUse:${use.name} hook error: [${rule}]`) &&
  debug.includes(`returned permissionDecision: deny (reason: [${rule}]`);

const commandOf = (use) =>
  String(use.input.command ?? use.input.file_path ?? "");

const sandboxDenied = (use, result, experiment, observed) =>
  result !== undefined &&
  result.error &&
  commandOf(use) === fill(experiment.exact, observed.values) &&
  new RegExp(experiment.evidence, "u").test(result.text);

const succeeded = (result, experiment) =>
  result !== undefined &&
  !result.error &&
  new RegExp(experiment.evidence, "u").test(result.text);

const judgeTool = (experiment, observed) => {
  const session = observed.sessions[experiment.probe.session];
  const events = session?.events ?? [];
  const debug = session?.debug ?? "";
  const uses = matchingUses(events, experiment.probe);
  if (uses.length === 0) {
    return unsettled("모델이 시도하지 않았다");
  }
  const results = uses.map((use) => ({ use, result: resultOf(events, use) }));
  const changed = changedTargets(experiment, observed);
  if (experiment.signal === "success") {
    const passing = results.find(({ result }) => succeeded(result, experiment));
    return decided(
      passing !== undefined,
      shortened((passing ?? results[0]).result?.text),
    );
  }
  const denied = ({ use, result }) =>
    experiment.signal === "hook"
      ? hookDenied(use, result, experiment.rule, debug)
      : experiment.signal === "hook-or-sandbox"
        ? hookDenied(use, result, experiment.rule, debug) ||
          sandboxDenied(use, result, experiment, observed)
        : sandboxDenied(use, result, experiment, observed);
  const leaked = results.filter((item) => !denied(item));
  if (changed.length > 0) {
    return decided(false, `대상이 바뀌었다: ${changed.join(", ")}`);
  }
  return decided(
    leaked.length === 0,
    leaked.length === 0
      ? `${results.length}회 모두 거부: ${shortened(results[0].result?.text)}`
      : `${results.length}회 중 ${leaked.length}회 거부되지 않음: ${shortened(commandOf(leaked[0].use))}`,
  );
};

const statusOf = (text) => /status (\d{3})/u.exec(text ?? "")?.[1];

const reached = (status) =>
  status !== undefined && status !== "000" && !status.startsWith("4");

const judgeNetwork = (experiment, observed) => {
  const session = observed.sessions[experiment.probe.session];
  const events = session?.events ?? [];
  const exact = (template) => (use) =>
    commandOf(use) === fill(template, observed.values);
  const allowed = matchingUses(
    events,
    experiment.probe,
    experiment.probe.allowed,
  )
    .filter(exact(experiment.allowedCommand))
    .map((use) => statusOf(resultOf(events, use)?.text));
  const denied = matchingUses(events, experiment.probe, experiment.probe.denied)
    .filter(exact(experiment.deniedCommand))
    .map((use) => statusOf(resultOf(events, use)?.text));
  if (allowed.length === 0 || denied.length === 0) {
    return unsettled("모델이 두 연결을 모두 시도하지 않았다");
  }
  if (!allowed.some(reached)) {
    return unsettled(
      `허용 도메인도 닿지 않았다(status ${allowed.join(",")}). 네트워크를 확인하고 다시 돌린다`,
    );
  }
  return decided(
    !denied.some(reached),
    `허용 status ${allowed.join(",")}, 금지 status ${denied.join(",")}`,
  );
};

const judgeToolList = (experiment, observed) => {
  const init = (observed.sessions[experiment.probe.session]?.events ?? []).find(
    (event) => event.type === "system" && event.subtype === "init",
  );
  if (init === undefined) {
    return unsettled("세션 시작 기록이 없다");
  }
  const present = (init.tools ?? []).includes(experiment.probe.absent);
  return decided(
    !present,
    present
      ? `${experiment.probe.absent}가 도구 목록에 있다`
      : `${experiment.probe.absent}가 도구 목록에 없다`,
  );
};

const debugLine = (debug, pattern) =>
  shortened(debug.split("\n").find((line) => pattern.test(line)));

const judgeDebug = (experiment, observed) => {
  const session = observed.sessions[experiment.probe.session];
  if (session?.markers?.[experiment.probe.marker] !== true) {
    return unsettled("변경을 넣을 시점(모델의 표시 명령)이 오지 않았다");
  }
  const evidence = new RegExp(fill(experiment.evidence, observed.values), "u");
  const debug = session.debug ?? "";
  return decided(
    evidence.test(debug),
    evidence.test(debug)
      ? debugLine(debug, evidence)
      : "디버그 기록에 차단이 없다",
  );
};

const judgeToolDebug = (experiment, observed) => {
  const session = observed.sessions[experiment.probe.session];
  const uses = matchingUses(session?.events ?? [], experiment.probe);
  if (uses.length === 0) {
    return unsettled("모델이 시도하지 않았다");
  }
  const evidence = new RegExp(fill(experiment.evidence, observed.values), "u");
  const debug = session.debug ?? "";
  return decided(
    evidence.test(debug),
    evidence.test(debug)
      ? debugLine(debug, evidence)
      : "디버그 기록에 그 시도의 훅 결과가 없다",
  );
};

const agentOf = (events, use) =>
  events.find(
    (event) =>
      event.type === "system" &&
      event.subtype === "task_started" &&
      event.tool_use_id === use.id,
  )?.task_id;

const windowOf = (events, use) => {
  const stamps = events
    .filter((event) => parentOf(event) === use.id && event.timestamp)
    .map((event) => event.timestamp)
    .sort();
  return { start: stamps[0], end: resultOf(events, use)?.timestamp };
};

const debugWithin = (debug, window) =>
  debug
    .split("\n")
    .filter(
      (line) =>
        window.start !== undefined &&
        window.end !== undefined &&
        line.slice(0, 24) >= window.start &&
        line.slice(0, 24) <= window.end,
    );

const launchedAgent = (experiment, observed) => {
  const session = observed.sessions[experiment.probe.session];
  const events = session?.events ?? [];
  const use = matchingUses(events, experiment.probe)[0];
  const agent = use === undefined ? undefined : agentOf(events, use);
  return { session, events, use, agent };
};

const judgeAgentContext = (experiment, observed) => {
  const { session, agent } = launchedAgent(experiment, observed);
  if (agent === undefined) {
    return unsettled("모델이 서브에이전트를 띄우지 않았다");
  }
  const expected = fill(experiment.evidence, { ...observed.values, agent });
  const found = (session.debug ?? "").includes(expected);
  return decided(found, found ? expected : `${expected}가 디버그 기록에 없다`);
};

const recordFits = (record, expected) =>
  Object.entries(expected).every(([key, value]) =>
    key === "minBlocks" ? (record.blocks ?? 0) >= value : record[key] === value,
  );

const recordOf = (observed, kind, agent) =>
  observed.records.find((record) => record.name === `${kind}-${agent}.json`);

const judgeStopReason = (experiment, observed) => {
  const { session, events, use, agent } = launchedAgent(experiment, observed);
  if (agent === undefined) {
    return unsettled("모델이 서브에이전트를 띄우지 않았다");
  }
  const record = recordOf(observed, experiment.probe.recordKind, agent);
  if (record === undefined) {
    return unsettled(`${experiment.probe.recordKind} 기록이 없다`);
  }
  const blocks = debugWithin(session.debug ?? "", windowOf(events, use)).filter(
    (line) =>
      /SubagentStop \(SubagentStop\) success:.{0,30}decision.{0,8}block/u.test(
        line,
      ),
  );
  const evidence = new RegExp(experiment.evidence, "u");
  const forbidden = new RegExp(experiment.forbid ?? "(?!)", "u");
  const reasonsFit =
    blocks.length > 0 &&
    blocks.every((line) => evidence.test(line) && !forbidden.test(line));
  const fits = recordFits(record.data, experiment.record) && reasonsFit;
  return decided(
    fits,
    `상태=${record.data.status ?? "없음"}, 차단=${record.data.blocks ?? 0}, 차단 이유 ${blocks.length}건${reasonsFit ? " 일치" : " 불일치"}`,
  );
};

const judgeAgentRecord = (experiment, observed) => {
  const { events, agent } = launchedAgent(experiment, observed);
  if (agent === undefined) {
    return unsettled("모델이 서브에이전트를 띄우지 않았다");
  }
  const attempted =
    matchingUses(
      events,
      {
        ...experiment.probe,
        agent: "subagent",
        tool: experiment.probe.childTool,
      },
      experiment.probe.childInput,
    ).length > 0;
  const record = recordOf(observed, experiment.probe.recordKind, agent);
  if (!attempted || record === undefined) {
    return unsettled(
      attempted
        ? `${experiment.probe.recordKind} 기록이 없다`
        : "서브에이전트가 시도하지 않았다",
    );
  }
  return decided(
    recordFits(record.data, experiment.record),
    `상태=${record.data.status ?? "없음"}, 차단=${record.data.blocks ?? 0}`,
  );
};

const JUDGES = {
  tool: judgeTool,
  network: judgeNetwork,
  "tool-list": judgeToolList,
  debug: judgeDebug,
  "tool-debug": judgeToolDebug,
  "agent-context": judgeAgentContext,
  "stop-reason": judgeStopReason,
  "agent-record": judgeAgentRecord,
};

export const judge = (experiment, observed) => {
  const decide = JUDGES[experiment.probe.kind];
  const outcome =
    decide === undefined
      ? decided(false, `모르는 실험 종류 ${experiment.probe.kind}`)
      : decide(experiment, observed);
  return {
    checked: (experiment.unchanged ?? []).map((target) =>
      fill(target, observed.values),
    ),
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
  const unsettledRows = rows.filter(
    (row) => row.verdict === VERDICT.unsettled,
  ).length;
  return {
    passed: failed === 0 && unsettledRows === 0,
    failed,
    unsettled: unsettledRows,
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
      : `claude-regression: 실패 (실패 ${summary.failed}건, 미확정 ${summary.unsettled}건)${summary.unsettled > 0 ? ". 미확정은 모델이 시도하지 않았거나 대조군이 닿지 않은 실험이다. just claude-regression을 다시 돌린다" : ""}`,
  ].join("\n");
};

const RECORD_FIELDS = {
  implementer: ["blocks", "status"],
  reviewer: ["minBlocks"],
};

const TOOL_PROBE = ["agent", "input", "kind", "session", "step", "tool"];

const PROBE_FIELDS = {
  tool: TOOL_PROBE,
  "tool-debug": TOOL_PROBE,
  "agent-context": TOOL_PROBE,
  "stop-reason": [...TOOL_PROBE, "recordKind"].sort(),
  "agent-record": [
    ...TOOL_PROBE,
    "childInput",
    "childTool",
    "recordKind",
  ].sort(),
  network: [
    "agent",
    "allowed",
    "controlStep",
    "denied",
    "kind",
    "session",
    "step",
    "tool",
  ],
  "tool-list": ["absent", "kind", "session"],
  debug: ["kind", "marker", "session"],
};

const BASE_FIELDS = ["expected", "id", "probe", "source", "title"];

const SIGNAL_FIELDS = {
  hook: ["rule", "signal"],
  sandbox: ["evidence", "exact", "signal"],
  "hook-or-sandbox": ["evidence", "exact", "rule", "signal"],
  success: ["evidence", "signal"],
};

const experimentFields = (experiment) => {
  const kind = experiment.probe?.kind;
  const extra =
    kind === "tool"
      ? [
          ...(SIGNAL_FIELDS[experiment.signal] ?? ["signal"]),
          ...("unchanged" in experiment ? ["unchanged"] : []),
        ]
      : kind === "network"
        ? ["allowedCommand", "deniedCommand"]
        : kind === "stop-reason"
          ? ["evidence", "forbid", "record"]
          : kind === "agent-record"
            ? ["record"]
            : kind === "tool-list"
              ? []
              : ["evidence"];
  return [...BASE_FIELDS, ...extra].sort();
};

const sameKeys = (object, keys) =>
  typeof object === "object" &&
  object !== null &&
  JSON.stringify(Object.keys(object).sort()) ===
    JSON.stringify([...keys].sort());

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
  if (!(kind in PROBE_FIELDS)) {
    return [`${experiment.id}: 모르는 실험 종류`];
  }
  const session = sessions.find((item) => item.id === experiment.probe.session);
  const prompt = session?.prompt ?? [];
  const stepLine = (step) => prompt[step] ?? "";
  return [
    ...(sameKeys(experiment, experimentFields(experiment))
      ? []
      : [
          `${experiment.id}: 실험 칸이 ${experimentFields(experiment).join(", ")}가 아니다`,
        ]),
    ...(sameKeys(experiment.probe, PROBE_FIELDS[kind])
      ? []
      : [
          `${experiment.id}: probe 칸이 ${PROBE_FIELDS[kind].join(", ")}가 아니다`,
        ]),
    ...(session === undefined ? [`${experiment.id}: 세션이 없다`] : []),
    ...["step", "controlStep"]
      .filter((key) => key in experiment.probe)
      .filter(
        (key) =>
          !stepLine(experiment.probe[key]).startsWith(
            `${experiment.probe[key]}. `,
          ),
      )
      .map(
        (key) =>
          `${experiment.id}: 세션 지시에 ${experiment.probe[key]}단계가 없다`,
      ),
    ...("exact" in experiment &&
    !stepLine(experiment.probe.step).endsWith(experiment.exact)
      ? [`${experiment.id}: 지시한 명령이 exact와 다르다`]
      : []),
    ...(kind === "network" &&
    !(
      stepLine(experiment.probe.step).endsWith(experiment.deniedCommand) &&
      stepLine(experiment.probe.controlStep).endsWith(experiment.allowedCommand)
    )
      ? [`${experiment.id}: 지시한 연결 명령이 다르다`]
      : []),
    ...(kind === "debug" && session?.configMarker === undefined
      ? [`${experiment.id}: 세션에 변경 시점 표시가 없다`]
      : []),
    ...(["stop-reason", "agent-record"].includes(kind) &&
    !sameKeys(
      experiment.record,
      RECORD_FIELDS[experiment.probe.recordKind] ?? [],
    )
      ? [`${experiment.id}: record 칸이 기록 종류와 맞지 않는다`]
      : []),
  ];
};

export const specProblems = (spec) => {
  if (!sameKeys(spec, ["experiments", "sessions", "setup"])) {
    return ["experiments.json: 최상위는 setup, sessions, experiments다"];
  }
  const sessions = spec.sessions;
  const covers = (session, step) =>
    spec.experiments.some(
      (experiment) =>
        experiment.probe?.session === session.id &&
        (experiment.probe?.step === step ||
          experiment.probe?.controlStep === step),
    ) ||
    (session.configMarker !== undefined &&
      (session.prompt?.[step] ?? "").includes(session.configMarker));
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
      .filter((step) => !covers(session, step))
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
  const setup = spec.setup;
  const scripts = setup?.scripts ?? {};
  const worktrees = Array.isArray(setup?.worktrees) ? setup.worktrees : [];
  const lines = sessions.flatMap((session) => session.prompt ?? []);
  const setupProblems = [
    ...(sameKeys(setup, ["lock", "scripts", "worktrees"]) && setup.lock === "on"
      ? []
      : ["setup은 lock(on), scripts, worktrees다"]),
    ...worktrees
      .filter(
        (worktree) =>
          !(
            sameKeys(
              worktree,
              "commit" in worktree
                ? ["branch", "commit", "from", "name"]
                : ["branch", "from", "name"],
            ) &&
            (!("commit" in worktree) ||
              sameKeys(worktree.commit, ["content", "path"]))
          ),
      )
      .map(
        (worktree) => `작업 사본 ${worktree.branch ?? "?"}의 칸이 맞지 않는다`,
      ),
    ...[
      ...new Set(
        lines.flatMap((line) =>
          [...line.matchAll(/\{experiment\}\/([A-Za-z0-9.-]+)/gu)].map(
            (match) => match[1],
          ),
        ),
      ),
    ]
      .filter((name) => !(name in scripts))
      .map((name) => `setup.scripts에 ${name}가 없다`),
    ...[
      ...new Set(
        lines.flatMap((line) =>
          [...line.matchAll(/브랜치 `([^`]+)`/gu)].map((match) => match[1]),
        ),
      ),
    ]
      .filter(
        (branch) => !worktrees.some((worktree) => worktree.branch === branch),
      )
      .map((branch) => `setup.worktrees에 ${branch}가 없다`),
  ];
  const unusedScripts = Object.keys(scripts)
    .filter(
      (name) =>
        !sessions.some((session) =>
          (session.prompt ?? []).some((line) => line.includes(`/${name}`)),
        ),
    )
    .map((name) => `setup.scripts의 ${name}를 쓰는 단계가 없다`);
  return [
    ...setupProblems,
    ...sessionProblems,
    ...duplicates,
    ...missingR10,
    ...unusedScripts,
    ...spec.experiments.flatMap((experiment) =>
      experimentProblems(experiment, sessions),
    ),
  ];
};
