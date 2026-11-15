import assert from "node:assert/strict";
import { before, after, test } from "node:test";
import { componentsDom } from "./ds-components-dom.mjs";
let fixture;
before(async () => {
  fixture = await componentsDom(String.raw`
import {createElement as h} from "react";
import {render,cleanup,act,queries} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {Card,EmptyState,Dialog,StatusBar,Text} from DS_ENTRY;
import {text} from LIB_ENTRY;
window.cleanup=cleanup;window.act=act;window.user=userEvent.setup({document});
window.q=Object.fromEntries(Object.entries(queries).map(([name,query])=>[name,(...args)=>query(document.body,...args)]));
const title=text("app.title","ko"), label=text("ds.description","ko"), cancel=text("ds.close","ko");
window.show=(name,disabled,cancelDisabled=disabled)=>{cleanup();window.calls=[];const action={label,onPress:()=>window.calls.push("confirm"),disabled};const back={label:cancel,onPress:()=>window.calls.push("cancel"),disabled:cancelDisabled};
if(name==="Dialog")return render(h(Dialog,{open:"open",title,actions:{confirm:action,cancel:back}},h(Text,null,title)));
if(name==="Card")return render(h(Card,{title,actions:action},h(Text,null,title)));
if(name==="StatusBar")return render(h(StatusBar,{action},title));
return render(h(EmptyState,{title,action}));};
window.dialog=(mode)=>{cleanup();window.calls=[];const confirm={label,onPress:()=>window.calls.push("confirm")};const back={label:cancel,onPress:()=>window.calls.push("cancel")};const actions=mode==="none"?undefined:mode==="empty"?{}:mode==="confirm"?{confirm}:mode==="cancel"?{cancel:back}:{confirm,cancel:back};return render(h(Dialog,{open:"open",title,actions},h(Text,null,title)));};
`);
});
after(() => fixture?.close());
for (const name of ["Card", "EmptyState", "Dialog", "StatusBar"]) {
  test(`DS21 ${name} owns button name, default type and callback`, async () => {
    const w = fixture.open();
    for (const disabled of [undefined, "enabled", "disabled"]) {
      await w.act(() => w.show(name, disabled));
      const button = w.q.getByRole("button", {
        name: "설명을 확인해 주세요",
        exact: true,
      });
      assert.equal(button.getAttribute("type"), "button");
      assert.equal(
        button.classList.contains(
          name === "Dialog" ? "h-control-height" : "h-control-height-sm",
        ),
        true,
      );
      assert.equal(
        button.classList.contains(
          name === "Dialog" ? "bg-primary" : "bg-surface",
        ),
        true,
      );
      assert.equal(button.disabled, disabled === "disabled");
      await w.act(() => w.user.click(button));
      assert.deepEqual(
        Array.from(w.calls),
        disabled === "disabled" ? [] : ["confirm"],
      );
      if (name === "Dialog") {
        const cancel = w.q.getByRole("button", { name: "닫기", exact: true });
        assert.equal(cancel.classList.contains("bg-surface"), true);
        await w.act(() => w.user.click(cancel));
        assert.deepEqual(
          Array.from(w.calls),
          disabled === "disabled" ? [] : ["confirm", "cancel"],
        );
        assert.ok(w.q.getByRole("dialog"));
      }
    }
  });
}

test("DS21 Dialog keeps confirmation and cancellation independent", async () => {
  const w = fixture.open();
  for (const [confirmState, cancelState] of [
    ["enabled", "disabled"],
    ["disabled", "enabled"],
  ]) {
    await w.act(() => w.show("Dialog", confirmState, cancelState));
    const confirm = w.q.getByRole("button", {
      name: "설명을 확인해 주세요",
      exact: true,
    });
    const cancel = w.q.getByRole("button", { name: "닫기", exact: true });
    assert.equal(confirm.disabled, confirmState === "disabled");
    assert.equal(cancel.disabled, cancelState === "disabled");
    await w.act(() => w.user.click(confirm));
    await w.act(() => w.user.click(cancel));
    assert.deepEqual(
      Array.from(w.calls),
      confirmState === "enabled" ? ["confirm"] : ["cancel"],
    );
  }
});

test("DS21 Dialog starts on cancel and Enter cannot confirm implicitly", async () => {
  const w = fixture.open();
  await w.act(() => w.dialog("both"));
  const buttons = w.q.getAllByRole("button");
  assert.deepEqual(
    Array.from(buttons, (button) => button.textContent),
    ["닫기", "설명을 확인해 주세요"],
  );
  assert.equal(w.document.activeElement, buttons[0]);
  await w.act(() => w.user.keyboard("{Enter}"));
  assert.deepEqual(Array.from(w.calls), ["cancel"]);
  await w.act(() => w.user.keyboard("{Escape}"));
  assert.deepEqual(Array.from(w.calls), ["cancel"]);
  await w.act(() => w.user.tab());
  assert.equal(w.document.activeElement, buttons[1]);
  await w.act(() => w.user.keyboard("{Enter}"));
  assert.deepEqual(Array.from(w.calls), ["cancel", "confirm"]);
});
for (const mode of ["cancel", "confirm", "none", "empty"]) {
  test(`DS21 Dialog ${mode} action combination has no decorative empty footer`, async () => {
    const w = fixture.open();
    await w.act(() => w.dialog(mode));
    const buttons = w.q.queryAllByRole("button");
    const present = mode === "cancel" || mode === "confirm";
    assert.equal(buttons.length, present ? 1 : 0);
    assert.equal(w.document.querySelector("footer") !== null, present);
    if (present) {
      assert.equal(
        buttons[0].textContent,
        mode === "cancel" ? "닫기" : "설명을 확인해 주세요",
      );
      assert.equal(
        buttons[0].classList.contains(
          mode === "cancel" ? "bg-surface" : "bg-primary",
        ),
        true,
      );
      assert.equal(buttons[0].classList.contains("h-control-height"), true);
      await w.act(() => w.user.click(buttons[0]));
      assert.deepEqual(Array.from(w.calls), [mode]);
    }
  });
}
test("DS21 Card keeps its heading before its small action", async () => {
  const w = fixture.open();
  await w.act(() => w.show("Card"));
  const header = w.document.querySelector("header");
  assert.equal(header.children.length, 2);
  assert.equal(
    header.children[0].querySelector("h3").textContent,
    "주식 코파일럿",
  );
  assert.equal(
    header.children[1],
    w.q.getByRole("button", { name: "설명을 확인해 주세요", exact: true }),
  );
});
