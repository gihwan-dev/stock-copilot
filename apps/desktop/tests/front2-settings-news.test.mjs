import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { choose, fakeSource, okReply } from "./front2-fake.mjs";
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
    window.mount = (fake) => {
      mockCommands({ appVersion: () => responses.appVersion });
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

const calls = (w, name) => w.fakeCalls.filter((call) => call.name === name);
const firstArg = (w, name, index = 0) =>
  JSON.parse(calls(w, name)[index].args)[0];
const waitCalls = (w, name, count) =>
  w.waitFor(() => {
    if (calls(w, name).length !== count)
      throw new Error(`waiting for ${count} ${name}`);
  });

const switches = (w, label) =>
  Array.from(
    w.document.body.querySelectorAll(`[role="switch"][aria-label="${label}"]`),
  );

const inputs = (w, label) =>
  Array.from(w.document.body.querySelectorAll(`input[aria-label="${label}"]`));

const buttonAfter = (w, anchor, label) =>
  Array.from(w.document.body.querySelectorAll("button")).find(
    (node) =>
      node.textContent === label &&
      anchor.compareDocumentPosition(node) & w.Node.DOCUMENT_POSITION_FOLLOWING,
  );

const selectsByLabel = (w, label) =>
  Array.from(w.document.body.querySelectorAll("label"))
    .filter((node) => node.textContent === label && node.htmlFor !== "")
    .map((node) => w.document.getElementById(node.htmlFor));

test("N-14: sources show their switches and interval, Yonhap body reading cannot be turned on", async () => {
  const w = bundle.open();
  w.mount(w.makeFake());
  await until(w, "수집 소스");
  const body = w.document.body.textContent;
  for (const name of ["KRX KIND 공시", "한국경제", "연합뉴스", "SEC EDGAR"])
    assert.ok(body.includes(name), name);
  const reading = switches(w, "AI 본문 읽기");
  assert.equal(reading.length, 4);
  assert.ok(
    reading[2].hasAttribute("disabled") ||
      reading[2].getAttribute("aria-disabled") === "true" ||
      reading[2].hasAttribute("data-disabled"),
  );
  assert.ok(body.includes("이 소스는 본문 읽기를 켤 수 없어요"));
  w.fireEvent.click(reading[2]);
  assert.equal(calls(w, "setNewsSource").length, 0);
  assert.equal(selectsByLabel(w, "수집 주기")[0].value, "2");
  assert.ok(!body.includes("마지막 성공"));
  w.cleanup();
});

test("N-14: switching a source, its body reading and interval sends the whole row", async () => {
  const w = bundle.open();
  w.mount(w.makeFake());
  await until(w, "수집 소스");
  w.fireEvent.click(switches(w, "수집")[3]);
  await waitCalls(w, "setNewsSource", 1);
  assert.deepEqual(firstArg(w, "setNewsSource"), {
    source: "SecEdgar",
    state: "on",
    body_reading: "off",
    interval_minutes: "60",
  });
  w.fireEvent.click(switches(w, "AI 본문 읽기")[1]);
  await waitCalls(w, "setNewsSource", 2);
  assert.equal(firstArg(w, "setNewsSource", 1).body_reading, "on");
  w.fireEvent.change(selectsByLabel(w, "수집 주기")[0], {
    target: { value: "10" },
  });
  await waitCalls(w, "setNewsSource", 3);
  assert.deepEqual(firstArg(w, "setNewsSource", 2), {
    source: "KindDisclosure",
    state: "on",
    body_reading: "off",
    interval_minutes: "10",
  });
  w.cleanup();
});

test("N-14: optional keys say how to turn the feature on and never stay on screen or in caches", async () => {
  const w = bundle.open();
  const { client } = w.mount(w.makeFake());
  await until(w, "선택 키");
  assert.ok(w.document.body.textContent.includes("키를 넣으면 켜집니다"));
  assert.ok(w.document.body.textContent.includes("키가 저장돼 있어요"));
  const dart = inputs(w, "DART 키")[0];
  assert.equal(dart.type, "password");
  w.fireEvent.change(dart, { target: { value: "dart-test-key-123" } });
  w.fireEvent.click(buttonAfter(w, dart, "저장"));
  await waitCalls(w, "setOptionalKey", 1);
  assert.deepEqual(JSON.parse(calls(w, "setOptionalKey")[0].args).slice(0, 2), [
    "dart",
    "dart-test-key-123",
  ]);
  await w.waitFor(() => {
    if (w.document.body.textContent.includes("키를 넣으면 켜집니다"))
      throw new Error("waiting for the saved state");
  });
  assert.equal(inputs(w, "DART 키")[0].value, "");
  const cached = JSON.stringify([
    client
      .getQueryCache()
      .getAll()
      .map((q) => q.state),
    client
      .getMutationCache()
      .getAll()
      .map((m) => m.state),
  ]);
  assert.ok(!cached.includes("dart-test-key-123"));
  assert.ok(!w.document.body.innerHTML.includes("dart-test-key-123"));
  w.fireEvent.change(inputs(w, "FRED 키")[0], {
    target: { value: "has space" },
  });
  w.fireEvent.click(buttonAfter(w, inputs(w, "FRED 키")[0], "저장"));
  await until(w, "키 형식이 맞지 않아요");
  assert.equal(calls(w, "setOptionalKey").length, 1);
  w.cleanup();
});

test("N-14: the SEC contact is validated, saved and cleared with null", async () => {
  const w = bundle.open();
  w.mount(w.makeFake());
  await until(w, "SEC 연락처");
  const field = () => inputs(w, "연락처 이메일")[0];
  w.fireEvent.change(field(), { target: { value: "not-an-email" } });
  w.fireEvent.click(buttonAfter(w, field(), "저장"));
  await until(w, "이메일 주소 형식으로");
  assert.equal(calls(w, "setSecContact").length, 0);
  w.fireEvent.change(field(), { target: { value: "me@example.com" } });
  w.fireEvent.click(buttonAfter(w, field(), "저장"));
  await waitCalls(w, "setSecContact", 1);
  assert.equal(firstArg(w, "setSecContact"), "me@example.com");
  w.fireEvent.change(field(), { target: { value: "" } });
  w.fireEvent.click(buttonAfter(w, field(), "저장"));
  await waitCalls(w, "setSecContact", 2);
  assert.equal(firstArg(w, "setSecContact", 1), null);
  w.cleanup();
});

test("N-14: wake criteria are sent whole, disclosures always wake", async () => {
  const w = bundle.open();
  w.mount(w.makeFake());
  await until(w, "AI를 깨우는 기준");
  assert.ok(w.document.body.textContent.includes("긴급·높음 등급 공시"));
  choose(w, "보유 종목의 하루 변동", "7");
  await waitCalls(w, "setWakeCriteria", 1);
  assert.deepEqual(firstArg(w, "setWakeCriteria"), {
    daily_move_percent: "7",
    news_per_hour: "5",
  });
  choose(w, "같은 종목 뉴스 (1시간)", "10");
  await waitCalls(w, "setWakeCriteria", 2);
  assert.equal(firstArg(w, "setWakeCriteria", 1).news_per_hour, "10");
  w.cleanup();
});

test("N-14: the alias table adds, removes and refuses bad rows", async () => {
  const w = bundle.open();
  w.mount(w.makeFake());
  await until(w, "종목 약칭");
  assert.equal(inputs(w, "약칭")[0].value, "SAMSUNG");
  const add = () =>
    Array.from(w.document.body.querySelectorAll("button")).find(
      (node) => node.textContent === "행 추가",
    );
  w.fireEvent.click(add());
  await w.waitFor(() => {
    if (inputs(w, "약칭").length !== 2) throw new Error("waiting for a row");
  });
  w.fireEvent.change(inputs(w, "약칭")[1], { target: { value: "HYNIX" } });
  w.fireEvent.change(inputs(w, "종목 코드")[1], { target: { value: "12" } });
  const save = () => buttonAfter(w, add(), "저장");
  w.fireEvent.click(save());
  await until(w, "6자리 종목 코드");
  assert.equal(calls(w, "setAliases").length, 0);
  w.fireEvent.change(inputs(w, "종목 코드")[1], {
    target: { value: "000660" },
  });
  w.fireEvent.click(save());
  await waitCalls(w, "setAliases", 1);
  assert.deepEqual(firstArg(w, "setAliases"), [
    { alias: "SAMSUNG", ticker: "005930" },
    { alias: "HYNIX", ticker: "000660" },
  ]);
  const removes = Array.from(w.document.body.querySelectorAll("button")).filter(
    (node) => node.textContent === "삭제",
  );
  w.fireEvent.click(removes[0]);
  w.fireEvent.click(save());
  await waitCalls(w, "setAliases", 2);
  assert.deepEqual(firstArg(w, "setAliases", 1), [
    { alias: "HYNIX", ticker: "000660" },
  ]);
  w.cleanup();
});

test("N-14: a failed load shows one sentence", async () => {
  const w = bundle.open();
  w.mount(
    w.makeFake({
      newsSettings: () => w.failedReply("retryable"),
      principles: () =>
        okReply(w, {
          text: "",
          cash_min_percent: "10",
          concentration_min_count: "5",
        }),
    }),
  );
  await until(w, "뉴스·데이터 설정을 불러오지 못했어요");
  w.cleanup();
});
