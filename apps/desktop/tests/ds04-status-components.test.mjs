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
window.showStatus=(props={},content="present",disabled="enabled")=>{cleanup();window.calls=[];const action=props.action==="present"?h(Button,{size:"control-height-sm",disabled,onClick:()=>window.calls.push("action")},text("ds.retry","ko")):undefined;render(h(StatusBar,{...props,action},content==="present"?text("ds.statusError","ko"):undefined));};
window.renderStory=(index,theme)=>{cleanup();document.documentElement.dataset.theme=theme;render(statusComponentStories[index].render());};
`);
});
after(() => fixture?.close());
for (const [tone, bg, color] of [
  ["neutral", "surface-sunken", "ink-muted"],
  ["primary", "primary-soft", "primary"],
  ["up", "up-soft", "up"],
  ["down", "down-soft", "down"],
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
    const bar = w.q.getByRole(tone === "error" ? "alert" : "status");
    assert.equal(bar.textContent, "토스에 연결하지 못했어요");
    assert.ok(
      bar.classList.contains(
        tone === "info" ? "bg-surface-sunken" : "bg-alert-soft",
      ),
    );
    assert.ok(
      bar.classList.contains(tone === "info" ? "border-line" : "border-alert"),
    );
    assert.equal(w.q.queryByRole(tone === "error" ? "status" : "alert"), null);
  });
}
test("DS04 StatusBar without a problem renders nothing including its action", () => {
  const w = fixture.open();
  for (const tone of [undefined, "warning", "error", "info"]) {
    w.showStatus({ tone, action: "present" }, "absent");
    assert.equal(w.document.body.firstElementChild.childElementCount, 0);
    assert.deepEqual(Array.from(w.calls), []);
  }
});
test("DS04 StatusBar defaults to warning and suppresses disabled child actions", async () => {
  const w = fixture.open();
  for (const disabled of ["enabled", "disabled"]) {
    w.showStatus({ action: "present" }, "present", disabled);
    assert.ok(w.q.getByRole("status").classList.contains("bg-alert-soft"));
    await w.act(() =>
      w.user.click(w.q.getByRole("button", { name: "다시 시도", exact: true })),
    );
    assert.deepEqual(
      Array.from(w.calls),
      disabled === "enabled" ? ["action"] : [],
    );
  }
});
test("DS04 all twenty status stories render their state in both themes", async () => {
  const w = fixture.open();
  assert.equal(w.stories.length, 20);
  assert.equal(
    Array.from(w.stories).filter((story) => story.component === "Badge").length,
    10,
  );
  for (const [index, story] of Array.from(w.stories).entries()) {
    assert.deepEqual(Array.from(story.themes), ["light", "dark"]);
    for (const theme of story.themes) {
      await w.act(() => w.renderStory(index, theme));
      if (story.state === "absent")
        assert.equal(w.document.body.firstElementChild.childElementCount, 0);
      else assert.ok(w.document.body.firstElementChild.firstElementChild);
    }
  }
});
test("DS04 alert tokens differ from both price directions in each theme", () => {
  const tokens = JSON.parse(
    readFileSync(
      new URL("../design-tokens/tokens.json", import.meta.url),
      "utf8",
    ),
  );
  for (const theme of ["light", "dark"]) {
    const value = (name) =>
      tokens.color.tokens.find((token) => token.name === name).value[theme];
    assert.notEqual(value("alert"), value("up"));
    assert.notEqual(value("alert"), value("down"));
    assert.notEqual(value("alert-soft"), value("up-soft"));
  }
});
