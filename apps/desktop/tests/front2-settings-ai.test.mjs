import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { choose, fakeSource, okReply, selectByLabel } from "./front2-fake.mjs";
import { screenDom } from "./screen-dom.mjs";
import { openAdvancedSource } from "./settings-advanced.mjs";

let bundle;

before(async () => {
  bundle = await screenDom(`
    import { createElement as h } from "react";
    import { render, fireEvent, waitFor, cleanup } from "@testing-library/react";
    import { App } from APP_ENTRY;
    import { createAppQueryClient } from QUERY_ENTRY;
    import { mockCommands } from MOCK_IPC_ENTRY;
    import { responses } from RESPONSES_ENTRY;
    ${fakeSource}
  ${openAdvancedSource}
    window.cleanup = cleanup;
    window.waitFor = waitFor;
    window.fireEvent = fireEvent;
    window.opened = [];
    window.mount = (fake) => {
      mockCommands({
        appVersion: () => responses.appVersion,
        openExternal: ({ request }) => {
          window.opened.push(request.url);
          return responses.openExternal;
        },
      });
      const client = createAppQueryClient();
      window.openAdvanced();
      const view = render(h(App, { initialPath: "/settings", queryClient: client, settingsClient: fake }));
      return { view, client };
    };
  `);
});

after(() => bundle.close());

const until = async (w, phrase) =>
  w.waitFor(() => {
    if (!w.document.body.textContent.includes(phrase))
      throw new Error(`waiting for ${phrase}`);
  });

const button = (w, label) =>
  Array.from(w.document.body.querySelectorAll("button")).filter(
    (node) => node.textContent === label,
  );

const calls = (w, name) => w.fakeCalls.filter((call) => call.name === name);
const firstArg = (w, name, index = 0) =>
  JSON.parse(calls(w, name)[index].args)[0];

const allSelects = (w, label) =>
  Array.from(w.document.body.querySelectorAll("label"))
    .filter((node) => node.textContent === label && node.htmlFor !== "")
    .map((node) => w.document.getElementById(node.htmlFor));

const buttonAfter = (w, anchor, label) =>
  Array.from(w.document.body.querySelectorAll("button")).find(
    (node) =>
      node.textContent === label &&
      anchor.compareDocumentPosition(node) & w.Node.DOCUMENT_POSITION_FOLLOWING,
  );

const fieldsByLabel = (w, label) =>
  Array.from(w.document.body.querySelectorAll(`input[aria-label="${label}"]`));

test("AI-10: every task shows its model and effort, the budget defaults to 70% and usage is shown", async () => {
  const w = bundle.open();
  w.mount(w.makeFake());
  await until(w, "작업별 모델");
  const body = w.document.body.textContent;
  for (const name of [
    "채팅",
    "확률 추정",
    "심층 연구",
    "브리핑과 리뷰",
    "뉴스 분류와 요약",
    "웹 수집",
  ])
    assert.ok(body.includes(name), name);
  const models = allSelects(w, "모델");
  assert.equal(models.length, 6);
  assert.equal(models[0].value, "opus");
  assert.equal(models[4].value, "haiku");
  assert.equal(allSelects(w, "추론 강도")[1].value, "high");
  assert.equal(
    selectByLabel(w, "예약 작업이 쓸 수 있는 구독 한도").value,
    "70",
  );
  assert.ok(body.includes("지금까지 쓴 한도"));
  assert.ok(body.includes("42.5"));
  assert.ok(!/제공자|Codex|MCP|세션|격리|큐|보정/u.test(body));
  w.cleanup();
});

test("AI-10: changing a model sends only that task and touches nothing else", async () => {
  const w = bundle.open();
  w.mount(w.makeFake());
  await until(w, "작업별 모델");
  w.fireEvent.change(allSelects(w, "모델")[1], { target: { value: "sonnet" } });
  await until(w, "저장했어요");
  assert.deepEqual(firstArg(w, "setTaskModel"), {
    task: "estimate",
    model: "sonnet",
    effort: "high",
  });
  await w.waitFor(() => {
    if (allSelects(w, "모델")[1].value !== "sonnet")
      throw new Error("waiting for the new model");
  });
  const names = new Set(w.fakeCalls.map((call) => call.name));
  for (const name of ["deleteAllData", "setScheduledJob", "setBudgetPercent"])
    assert.ok(!names.has(name), name);
  w.fireEvent.change(allSelects(w, "추론 강도")[0], {
    target: { value: "max" },
  });
  await w.waitFor(() => {
    if (calls(w, "setTaskModel").length !== 2)
      throw new Error("waiting for effort change");
  });
  assert.deepEqual(firstArg(w, "setTaskModel", 1), {
    task: "chat",
    model: "opus",
    effort: "max",
  });
  w.cleanup();
});

test("AI-10: the budget ratio is sent as chosen", async () => {
  const w = bundle.open();
  w.mount(w.makeFake());
  await until(w, "예약 작업 한도");
  choose(w, "예약 작업이 쓸 수 있는 구독 한도", "50");
  await w.waitFor(() => {
    if (calls(w, "setBudgetPercent").length !== 1)
      throw new Error("waiting for budget");
  });
  assert.equal(firstArg(w, "setBudgetPercent"), "50");
  w.cleanup();
});

test("AI-10: the Claude folder is optional, validated and cleared with null", async () => {
  const w = bundle.open();
  w.mount(w.makeFake());
  await until(w, "Claude 설정 위치");
  const folder = () => fieldsByLabel(w, "설정 폴더")[0];
  const save = () => buttonAfter(w, folder(), "저장");
  w.fireEvent.change(folder(), { target: { value: "relative/path" } });
  w.fireEvent.click(save());
  await until(w, "/ 또는 ~로 시작하는 폴더 경로");
  assert.equal(calls(w, "setClaudeConfigDir").length, 0);
  w.fireEvent.change(folder(), { target: { value: "~/work/claude" } });
  w.fireEvent.click(save());
  await w.waitFor(() => {
    if (calls(w, "setClaudeConfigDir").length !== 1)
      throw new Error("waiting for folder save");
  });
  assert.equal(firstArg(w, "setClaudeConfigDir"), "~/work/claude");
  w.fireEvent.change(folder(), { target: { value: "" } });
  w.fireEvent.click(save());
  await w.waitFor(() => {
    if (calls(w, "setClaudeConfigDir").length !== 2)
      throw new Error("waiting for folder clear");
  });
  assert.equal(firstArg(w, "setClaudeConfigDir", 1), null);
  w.cleanup();
});

test("AI-10: the training settings shortcut opens the fixed Anthropic page", async () => {
  const w = bundle.open();
  w.mount(w.makeFake());
  await until(w, "데이터 전송 안내");
  assert.ok(w.document.body.textContent.includes("Anthropic 서버에 보관돼요"));
  w.fireEvent.click(button(w, "Anthropic 학습 설정 열기")[0]);
  await w.waitFor(() => {
    if (w.opened.length !== 1) throw new Error("waiting for open_external");
  });
  assert.equal(w.opened[0], "https://claude.ai/settings/data-privacy-controls");
  w.cleanup();
});

test("J-10: scheduled jobs can be switched off and retimed, bad times are refused", async () => {
  const w = bundle.open();
  w.mount(w.makeFake());
  await until(w, "아침 브리핑");
  const toggle = Array.from(
    w.document.body.querySelectorAll('[role="switch"]'),
  ).find((node) =>
    (
      node.getAttribute("aria-label") ?? node.parentElement.textContent
    ).includes("마감 리뷰"),
  );
  assert.ok(toggle);
  w.fireEvent.click(toggle);
  await w.waitFor(() => {
    if (calls(w, "setScheduledJob").length !== 1)
      throw new Error("waiting for the switch");
  });
  assert.deepEqual(firstArg(w, "setScheduledJob"), {
    job: "closing-review",
    state: "off",
    time: "15:50",
  });
  const times = fieldsByLabel(w, "시각");
  assert.equal(times.length, 4);
  assert.equal(times[0].value, "08:20");
  w.fireEvent.change(times[0], { target: { value: "25:00" } });
  const timeSave = buttonAfter(w, times[0], "저장");
  w.fireEvent.click(timeSave);
  await until(w, "24시간 형식으로");
  assert.equal(calls(w, "setScheduledJob").length, 1);
  w.fireEvent.change(fieldsByLabel(w, "시각")[0], {
    target: { value: "07:45" },
  });
  w.fireEvent.click(timeSave);
  await w.waitFor(() => {
    if (calls(w, "setScheduledJob").length !== 2)
      throw new Error("waiting for the time");
  });
  assert.deepEqual(firstArg(w, "setScheduledJob", 1), {
    job: "morning-briefing",
    state: "on",
    time: "07:45",
  });
  assert.ok(w.document.body.textContent.includes("정해진 시각 없이"));
  w.cleanup();
});

test("AI-10: a failed load shows one sentence", async () => {
  const w = bundle.open();
  w.mount(
    w.makeFake({
      aiSettings: () => w.failedReply("retryable"),
      scheduledJobs: () => okReply(w, { jobs: [] }),
    }),
  );
  await until(w, "AI 설정을 불러오지 못했어요");
  w.cleanup();
});
