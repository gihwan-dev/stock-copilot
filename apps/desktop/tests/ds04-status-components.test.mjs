import assert from "node:assert/strict";
import { before, after, test } from "node:test";
import { readFileSync } from "node:fs";
import { componentsDom } from "./ds-components-dom.mjs";
let fixture;
before(async () => {
  fixture = await componentsDom(String.raw`
import {createElement as h} from "react";
import {render,cleanup,act,queries} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {Badge,StatusBar,Button} from DS_ENTRY;
import {text} from LIB_ENTRY;
import {statusComponentStories} from STORY_ENTRY;
window.cleanup=cleanup;window.act=act;window.calls=[];
window.q=Object.fromEntries(Object.entries(queries).map(([name,query])=>[name,(...args)=>query(document.body,...args)]));
window.user=userEvent.setup({document});window.stories=statusComponentStories;
window.showBadge=(props={})=>{cleanup();render(h(Badge,props,text("ds.badgeNeutral","ko")));};
window.showStatus=(props={},content="present",disabled="enabled")=>{cleanup();window.calls=[];const action=props.action==="present"?{label:text("ds.retry","ko"),disabled,onPress:()=>window.calls.push("action")}:undefined;render(h(StatusBar,{...props,action},content==="present"?text("ds.statusError","ko"):undefined));};
window.transition=()=>{cleanup();const view=render(h(StatusBar));const status=document.querySelector('[role="status"]'),alert=document.querySelector('[role="alert"]');view.rerender(h(StatusBar,{tone:"warning"},text("ds.statusWarning","ko")));const warning=status.textContent;view.rerender(h(StatusBar,{tone:"error"},text("ds.statusError","ko")));window.transitionResult={sameStatus:document.querySelector('[role="status"]')===status,sameAlert:document.querySelector('[role="alert"]')===alert,warning,emptyStatus:status.textContent,error:alert.textContent};};
window.renderStory=(index,theme)=>{cleanup();document.documentElement.dataset.theme=theme;render(statusComponentStories[index].render());};
`);
});
after(() => fixture?.close());
for (const [tone, bg, color] of [
  ["neutral", "surface-sunken", "ink-muted"],
  ["primary", "primary-soft", "primary"],
  ["alert", "alert-soft", "alert"],
]) {
  test(`DS04 Badge ${tone} maps only to its design tokens in both fonts`, () => {
    const w = fixture.open();
    for (const mono of ["normal", "mono"]) {
      w.showBadge({ tone, mono });
      const label = w.q.getByText("보유 종목", { exact: true });
      const badge = label.closest("[data-badge]");
      assert.ok(badge.classList.contains("bg-" + bg));
      assert.ok(badge.classList.contains("text-" + color));
      assert.equal(label.classList.contains("font-mono"), mono === "mono");
      assert.equal(w.q.queryByRole("button"), null);
    }
  });
}
test("DS04 Badge defaults to neutral and normal font", () => {
  const w = fixture.open();
  w.showBadge();
  assert.ok(
    w.document
      .querySelector("[data-badge]")
      .classList.contains("bg-surface-sunken"),
  );
  assert.ok(
    w.q.getByText("보유 종목", { exact: true }).classList.contains("font-sans"),
  );
});
for (const tone of ["warning", "error", "info"]) {
  test(`DS04 StatusBar ${tone} has the exact role, sentence and design tokens`, () => {
    const w = fixture.open();
    w.showStatus({ tone });
    const region = w.q.getByRole(tone === "error" ? "alert" : "status");
    const bar = region.firstElementChild;
    assert.equal(bar.textContent, "토스에 연결하지 못했어요");
    assert.ok(
      bar.classList.contains(
        tone === "info" ? "bg-surface-sunken" : "bg-alert-soft",
      ),
    );
    assert.ok(
      bar.classList.contains(tone === "info" ? "border-line" : "border-alert"),
    );
    assert.equal(
      w.q.getByRole(tone === "error" ? "status" : "alert").childElementCount,
      0,
    );
  });
}
test("DS04 StatusBar without a problem renders nothing including its action", () => {
  const w = fixture.open();
  for (const tone of [undefined, "warning", "error", "info"]) {
    w.showStatus({ tone, action: "present" }, "absent");
    assert.equal(w.q.getByRole("status").childElementCount, 0);
    assert.equal(w.q.getByRole("alert").childElementCount, 0);
    assert.deepEqual(Array.from(w.calls), []);
  }
});
test("DS04 StatusBar defaults to warning and suppresses disabled child actions", async () => {
  const w = fixture.open();
  for (const disabled of ["enabled", "disabled"]) {
    w.showStatus({ action: "present" }, "present", disabled);
    assert.ok(
      w.q
        .getByRole("status")
        .firstElementChild.classList.contains("bg-alert-soft"),
    );
    await w.act(() =>
      w.user.click(w.q.getByRole("button", { name: "다시 시도", exact: true })),
    );
    assert.deepEqual(
      Array.from(w.calls),
      disabled === "enabled" ? ["action"] : [],
    );
  }
});
test("DS04 all sixteen status stories render their state in both themes", async () => {
  const w = fixture.open();
  assert.equal(w.stories.length, 16);
  assert.equal(
    Array.from(w.stories).filter((story) => story.component === "Badge").length,
    6,
  );
  for (const [index, story] of Array.from(w.stories).entries()) {
    assert.deepEqual(Array.from(story.themes), ["light", "dark"]);
    for (const theme of story.themes) {
      await w.act(() => w.renderStory(index, theme));
      if (story.state === "absent") {
        assert.equal(w.q.getByRole("status").childElementCount, 0);
        assert.equal(w.q.getByRole("alert").childElementCount, 0);
      } else if (story.component === "Badge") {
        const tone = ["neutral", "primary", "alert"][Math.floor(index / 2)];
        const badge = w.document.querySelector("[data-badge]");
        assert.ok(
          badge.classList.contains(
            "bg-" +
              {
                neutral: "surface-sunken",
                primary: "primary-soft",
                alert: "alert-soft",
              }[tone],
          ),
        );
        assert.ok(
          badge.classList.contains(
            "text-" +
              { neutral: "ink-muted", primary: "primary", alert: "alert" }[
                tone
              ],
          ),
        );
        assert.equal(
          badge.textContent,
          { neutral: "보유 종목", primary: "앱 계산", alert: "값 오류" }[tone],
        );
      } else {
        const tone =
          index === 15
            ? "warning"
            : ["warning", "error", "info"][Math.floor((index - 6) / 2)];
        const region = w.q.getByRole(tone === "error" ? "alert" : "status");
        assert.ok(
          region.firstElementChild.classList.contains(
            tone === "info" ? "bg-surface-sunken" : "bg-alert-soft",
          ),
        );
        assert.equal(
          region.textContent,
          {
            warning: "자료가 오래됐어요",
            error: "토스에 연결하지 못했어요",
            info: "오늘은 휴장이에요",
          }[tone] + (index < 12 && index % 2 === 0 ? "다시 시도" : ""),
        );
        assert.equal(
          w.q.getByRole(tone === "error" ? "status" : "alert")
            .childElementCount,
          0,
        );
      }
    }
  }
});
const lab = (hex) => {
  const rgb = hex
    .slice(1)
    .match(/../g)
    .map((v) => parseInt(v, 16) / 255)
    .map((v) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
  const xyz = [
    [0.4124564, 0.3575761, 0.1804375],
    [0.2126729, 0.7151522, 0.072175],
    [0.0193339, 0.119192, 0.9503041],
  ]
    .map(
      (row, i) =>
        row.reduce((a, v, j) => a + v * rgb[j], 0) / [0.95047, 1, 1.08883][i],
    )
    .map((v) => (v > 216 / 24389 ? Math.cbrt(v) : (v * 841) / 108 + 4 / 29));
  return [116 * xyz[1] - 16, 500 * (xyz[0] - xyz[1]), 200 * (xyz[1] - xyz[2])];
};
test("DS04 alert differs from neutral and primary by at least ten CIE76 units", () => {
  const tokens = JSON.parse(
    readFileSync(
      new URL("../design-tokens/tokens.json", import.meta.url),
      "utf8",
    ),
  );
  for (const theme of ["light", "dark"]) {
    const value = (name) =>
      lab(tokens.color.tokens.find((t) => t.name === name).value[theme]);
    for (const name of ["ink-muted", "primary"]) {
      const left = value("alert"),
        right = value(name);
      assert.ok(
        Math.hypot(...left.map((v, i) => v - right[i])) >= 10,
        theme + name,
      );
    }
  }
});

test("DS04 live region nodes persist from empty through warning and error", () => {
  const w = fixture.open();
  w.transition();
  assert.equal(w.transitionResult.sameStatus, true);
  assert.equal(w.transitionResult.sameAlert, true);
  assert.equal(w.transitionResult.warning, "자료가 오래됐어요");
  assert.equal(w.transitionResult.emptyStatus, "");
  assert.equal(w.transitionResult.error, "토스에 연결하지 못했어요");
});
