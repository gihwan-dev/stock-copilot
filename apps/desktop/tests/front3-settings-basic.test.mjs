import assert from "node:assert/strict";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { choose, fakeSource, selectByLabel } from "./front2-fake.mjs";
import { desktopRoot } from "./paths.mjs";
import { screenDom } from "./screen-dom.mjs";

const profileEntry = JSON.stringify(
  join(desktopRoot, "src", "shared/ipc/profile-schema.ts"),
);

const defaultProfile = {
  horizon: "weeks-months",
  risk: "neutral",
  excluded_kinds: [],
  stock_cap_percent: "20",
  sector_cap_percent: "40",
};

let bundle;

before(async () => {
  bundle = await screenDom(`
    import { createElement as h } from "react";
    import { render, fireEvent, waitFor, cleanup } from "@testing-library/react";
    import { App } from APP_ENTRY;
    import { createAppQueryClient } from QUERY_ENTRY;
    import { mockCommands } from MOCK_IPC_ENTRY;
    import { responses } from RESPONSES_ENTRY;
    import * as profile from ${profileEntry};
    ${fakeSource}
    window.cleanup = cleanup;
    window.waitFor = waitFor;
    window.fireEvent = fireEvent;
    const decodeProfile = (value, schema) =>
      Promise.resolve(settleWithSchema({ status: "ok", data: value }, schema));
    window.orderState = "off";
    window.basicFake = (overrides = {}) =>
      window.makeFake({
        investmentProfile: () => decodeProfile(${JSON.stringify(defaultProfile)}, profile.profileSchema),
        setInvestmentProfile: (p) => decodeProfile(p, profile.profileSchema),
        orderSwitch: () => decodeProfile({ state: window.orderState }, profile.orderSwitchSchema),
        requestOrderSwitch: (state) => decodeProfile({ state }, profile.orderSwitchSchema),
        ...overrides,
      });
    window.profileFailure = () =>
      Promise.resolve(settleWithSchema({ status: "error", error: { kind: "bug" } }, profile.profileSchema));
    window.orderReply = (state) => decodeProfile({ state }, profile.orderSwitchSchema);
    window.mount = (fake) => {
      mockCommands({ appVersion: () => responses.appVersion });
      const client = createAppQueryClient();
      return render(h(App, { initialPath: "/settings", queryClient: client, settingsClient: fake }));
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

const hiddenText = (w) =>
  Array.from(w.document.body.querySelectorAll("[hidden]"))
    .map((node) => node.textContent)
    .join("");

const visibleText = (w) => {
  const clone = w.document.body.cloneNode(true);
  clone.querySelectorAll("[hidden]").forEach((node) => node.remove());
  return clone.textContent;
};

const button = (w, label) =>
  Array.from(w.document.body.querySelectorAll("button")).find(
    (node) => node.textContent.trim() === label,
  );

const orderSwitch = (w) =>
  w.document.body.querySelector('[role="switch"][aria-label="주문 켜기"]');

test("U-23: the basic view shows only Toss, profile and order switch; the rest sits under a closed 고급", async () => {
  const w = bundle.open();
  w.mount(w.basicFake());
  await until(w, "토스증권에 연결돼 있어요");
  await w.waitFor(() => {
    if (orderSwitch(w) === null) throw new Error("waiting for the switch");
  });
  const shown = visibleText(w);
  for (const phrase of ["토스 연결", "투자 성향", "주문 켜기", "고급"])
    assert.ok(shown.includes(phrase), phrase);
  for (const phrase of ["투자 원칙", "토스 자료 갱신", "키를 넣으면 켜집니다"])
    assert.equal(shown.includes(phrase), false, phrase);
  assert.ok(hiddenText(w).includes("투자 원칙"));
  const trigger = button(w, "고급");
  assert.equal(trigger.getAttribute("aria-expanded"), "false");
  w.fireEvent.click(trigger);
  await w.waitFor(() => {
    if (!visibleText(w).includes("키를 넣으면 켜집니다"))
      throw new Error("waiting for advanced");
  });
  assert.equal(trigger.getAttribute("aria-expanded"), "true");
  w.cleanup();
});

test("U-23: profile defaults match the design and saving sends every field once", async () => {
  const w = bundle.open();
  w.mount(w.basicFake());
  await w.waitFor(() => {
    if (selectByLabel(w, "기간") === null) throw new Error("waiting");
  });
  assert.equal(selectByLabel(w, "기간").value, "weeks-months");
  assert.equal(selectByLabel(w, "위험 성향").value, "neutral");
  assert.equal(selectByLabel(w, "한 종목 비중 상한").value, "20");
  assert.equal(selectByLabel(w, "한 섹터 비중 상한").value, "40");
  choose(w, "기간", "days");
  choose(w, "위험 성향", "conservative");
  choose(w, "한 섹터 비중 상한", "30");
  const etf = w.document.body.querySelector(
    '[role="checkbox"][aria-label="ETF"]',
  );
  w.fireEvent.click(etf);
  assert.equal(calls(w, "setInvestmentProfile").length, 0);
  w.fireEvent.click(button(w, "저장"));
  await until(w, "저장했어요");
  assert.equal(calls(w, "setInvestmentProfile").length, 1);
  assert.deepEqual(JSON.parse(calls(w, "setInvestmentProfile")[0].args)[0], {
    horizon: "days",
    risk: "conservative",
    excluded_kinds: ["etf"],
    stock_cap_percent: "20",
    sector_cap_percent: "30",
  });
  w.cleanup();
});

test("U-23: the order switch shows only what the core answers", async () => {
  const w = bundle.open();
  w.orderState = "off";
  w.mount(w.basicFake({ requestOrderSwitch: () => w.orderReply("off") }));
  await w.waitFor(() => {
    if (orderSwitch(w) === null) throw new Error("waiting for the switch");
  });
  assert.equal(orderSwitch(w).getAttribute("aria-checked"), "false");
  w.fireEvent.click(orderSwitch(w));
  await w.waitFor(() => {
    if (calls(w, "requestOrderSwitch").length !== 1) throw new Error("waiting");
  });
  assert.equal(JSON.parse(calls(w, "requestOrderSwitch")[0].args)[0], "on");
  await w.waitFor(() => {
    if (orderSwitch(w).hasAttribute("data-disabled"))
      throw new Error("waiting");
  });
  assert.equal(orderSwitch(w).getAttribute("aria-checked"), "false");
  w.cleanup();
  const second = bundle.open();
  second.orderState = "off";
  second.mount(second.basicFake());
  await second.waitFor(() => {
    if (orderSwitch(second) === null) throw new Error("waiting");
  });
  second.fireEvent.click(orderSwitch(second));
  await second.waitFor(() => {
    if (orderSwitch(second).getAttribute("aria-checked") !== "true")
      throw new Error("waiting for on");
  });
  assert.ok(
    second.document.body.textContent.includes("승인하면 주문이 나가요"),
  );
  second.cleanup();
});

test("U-23: re-pasting Toss keys clears both fields and sends them once", async () => {
  const w = bundle.open();
  w.mount(
    w.basicFake({ submitTossKeys: () => w.connectionReply("invalid-key") }),
  );
  await until(w, "토스증권에 연결돼 있어요");
  const fields = Array.from(
    w.document.body.querySelectorAll('input[type="password"]'),
  ).slice(0, 2);
  w.fireEvent.change(fields[0], { target: { value: "client-test-1" } });
  w.fireEvent.change(fields[1], { target: { value: "secret-test-1" } });
  w.fireEvent.click(button(w, "키 다시 넣기"));
  assert.equal(fields[0].value, "");
  assert.equal(fields[1].value, "");
  await w.waitFor(() => {
    if (calls(w, "submitTossKeys").length !== 1) throw new Error("waiting");
  });
  w.cleanup();
});

test("U-23: an empty key never reaches the core and a failed profile load shows one sentence", async () => {
  const w = bundle.open();
  w.mount(w.basicFake({ investmentProfile: () => w.profileFailure() }));
  await until(w, "투자 성향을 불러오지 못했어요");
  w.fireEvent.click(button(w, "키 다시 넣기"));
  await until(w, "키 형식이 맞지 않아요");
  assert.equal(calls(w, "submitTossKeys").length, 0);
  w.cleanup();
});
