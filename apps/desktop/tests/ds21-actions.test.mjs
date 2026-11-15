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
if(name==="Card")return render(h(Card,{actions:action},h(Text,null,title)));
if(name==="StatusBar")return render(h(StatusBar,{action},title));
return render(h(EmptyState,{title,action}));};
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
