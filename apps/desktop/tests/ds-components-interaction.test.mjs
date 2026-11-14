import "@testing-library/react";
import "@testing-library/user-event";
import assert from "node:assert/strict";
import { before, after, test } from "node:test";
import { componentsDom } from "./ds-components-dom.mjs";
let fixture;
before(async () => {
  fixture = await componentsDom(String.raw`
import {createElement as h,useState} from "react";
import {render,cleanup,act,fireEvent,queries,configure} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {Button,IconButton,Checkbox,Switch,Disclosure,TextField,Select,SegmentedControl,Text,Heading,Icon} from DS_ENTRY;
import {baseComponentStories} from STORY_ENTRY;
import {inputCommit} from INPUT_ENTRY;
import {text,parseInputValue,emptyInputValue} from LIB_ENTRY;
configure({asyncWrapper:(callback)=>callback()});
const label=text("app.title","ko");
const value=(raw)=>{const parsed=parseInputValue(raw);if(parsed.kind!=="ok") throw Error("invalid fixture");return parsed.value;};
const options=[{value:value("day"),label:text("ds.day","ko")},{value:value("month"),label:text("ds.month","ko")},{value:value("year"),label:text("ds.year","ko")}];
window.cleanup=cleanup;window.act=act;window.fire=fireEvent;window.q=Object.fromEntries(Object.entries(queries).map(([name,query])=>[name,(...args)=>query(document.body,...args)]));
window.user=userEvent.setup({document});
window.show=(name,props={})=>{
cleanup();window.calls=[];window.submits=[];
const Host=()=>{
const [checked,setChecked]=useState(name==="Switch"?"off":"unchecked");
const [open,setOpen]=useState("closed");
const [field,setField]=useState(emptyInputValue);
const [selected,setSelected]=useState(options[0].value);
window.setField=(raw)=>setField(value(raw));
const click=()=>window.calls.push("clicked");
const change=(next)=>{window.calls.push(next);setChecked(next);};
const changeField=(next)=>{window.calls.push(next);setField(next);};
const choose=(next)=>{window.calls.push(next);setSelected(next);};
if(name==="Button") return h(Button,{onClick:click,...props},label);
if(name==="IconButton") return h(IconButton,{icon:"settings",label,onClick:click,...props});
if(name==="Checkbox"||name==="Switch") return h(name==="Checkbox"?Checkbox:Switch,{label,description:text("ds.description","ko"),checked,onChange:change,...props});
if(name==="Disclosure") return h(Disclosure,{title:label,open,onOpenChange:(next)=>{window.calls.push(next);setOpen(next);},...props},h(Text,null,text("ds.description","ko")));
if(name==="TextField") return h("form",{onSubmit:(event)=>{event.preventDefault();window.submits.push("submit");}},h(TextField,{label,value:field,onChange:changeField,...props}));
if(name==="Select") return h(Select,{label,options,value:selected,onChange:choose,...props});
if(name==="SegmentedControl") return h(SegmentedControl,{label,options,value:selected,onChange:choose,...props});
if(name==="Heading") return h(Heading,props,label);
if(name==="Icon") return h(Icon,props);
return h(Text,props,label);
};render(h(Host));
};
window.stories=baseComponentStories.map(story=>({component:story.component,state:story.state,themes:story.themes}));
window.renderStories=(theme)=>{cleanup();document.documentElement.dataset.theme=theme;const view=render(h("div",null,...baseComponentStories.map((story,index)=>h("div",{"data-testid":"story-"+index,key:index},story.render()))));return baseComponentStories.map((story,index)=>{const node=view.getByTestId("story-"+index);const input=node.querySelector("input:not([type=hidden]),select");return {component:story.component,state:story.state,rendered:node.childElementCount>0,disabled:node.querySelector(":disabled")!==null,invalid:input?.getAttribute("aria-invalid"),alert:node.querySelector('[role="alert"]')!==null,selected:node.querySelector('[aria-checked="true"],[aria-pressed="true"]')!==null,open:node.querySelector('[aria-expanded="true"]')!==null};});};
window.commit=(composing,code,phase)=>{let prevented=false;const result=inputCommit({key:"Enter",nativeEvent:{isComposing:composing,keyCode:code},preventDefault:()=>{prevented=true;}},phase);return {result,prevented};};
window.clipboard=(mode)=>Object.defineProperty(navigator,"clipboard",{configurable:true,value:mode==="absent"?undefined:{readText:()=>mode==="deny"?Promise.reject(Error("denied")):Promise.resolve("  example-secret \n")}});
`);
});
after(() => fixture?.close());
const calls = (w) => Array.from(w.calls);
const action = (w, run) => w.act(run);

test("Button design variants, sizes, Tab/Enter/Space and disabled callbacks", async () => {
  const w = fixture.open();
  for (const variant of ["primary", "secondary", "ghost"])
    for (const size of ["control-height", "control-height-sm"]) {
      w.show("Button", { variant, size });
      const button = w.q.getByRole("button", {
        name: "주식 코파일럿",
        exact: true,
      });
      await action(w, () => w.user.tab());
      assert.equal(w.document.activeElement, button);
      await action(w, () => w.user.hover(button));
      await action(w, () => w.user.keyboard("{Enter} "));
      assert.deepEqual(calls(w), ["clicked", "clicked"]);
      assert.ok(button.classList.contains("h-" + size));
      w.show("Button", { variant, size, disabled: "disabled" });
      const disabled = w.q.getByRole("button", {
        name: "주식 코파일럿",
        exact: true,
      });
      assert.equal(disabled.disabled, true);
      await action(w, () => w.user.click(disabled));
      w.fire.click(disabled);
      assert.deepEqual(calls(w), []);
    }
});
test("IconButton exact name, pressed, expanded, focus tooltip and disabled", async () => {
  const w = fixture.open();
  w.show("IconButton", { pressed: "pressed", expanded: "open" });
  const button = w.q.getByRole("button", {
    name: "주식 코파일럿",
    exact: true,
  });
  assert.equal(button.getAttribute("aria-pressed"), "true");
  assert.equal(button.getAttribute("aria-expanded"), "true");
  await action(w, () => w.user.tab());
  assert.equal(
    (await w.q.findByRole("tooltip", { name: "주식 코파일럿", exact: true }))
      .textContent,
    "주식 코파일럿",
  );
  await action(w, () => w.user.keyboard(" "));
  assert.deepEqual(calls(w), ["clicked"]);
  w.show("IconButton", { disabled: "disabled", size: "control-height-sm" });
  const disabled = w.q.getByRole("button", {
    name: "주식 코파일럿",
    exact: true,
  });
  assert.equal(disabled.disabled, true);
  w.fire.click(disabled);
  assert.deepEqual(calls(w), []);
});
for (const name of ["Checkbox", "Switch"])
  test(`${name} keyboard toggle, controlled value and disabled`, async () => {
    const w = fixture.open();
    w.show(name);
    const role = name === "Switch" ? "switch" : "checkbox";
    const control = w.q.getByRole(role, { name: "주식 코파일럿", exact: true });
    assert.equal(control.getAttribute("aria-checked"), "false");
    await action(w, () => w.user.tab());
    assert.equal(w.document.activeElement, control);
    await action(w, () => w.user.keyboard(" "));
    assert.equal(control.getAttribute("aria-checked"), "true");
    assert.deepEqual(calls(w), [name === "Switch" ? "on" : "checked"]);
    await action(w, () => w.user.keyboard(" "));
    assert.equal(control.getAttribute("aria-checked"), "false");
    w.show(name, { disabled: "disabled" });
    w.fire.click(w.q.getByRole(role, { name: "주식 코파일럿", exact: true }));
    assert.deepEqual(calls(w), []);
  });
test("Disclosure rotates one glyph and Enter/Space relate the advanced panel", async () => {
  const w = fixture.open();
  w.show("Disclosure");
  const trigger = w.q.getByRole("button", {
    name: "주식 코파일럿",
    exact: true,
  });
  const glyph = trigger.querySelector("svg").innerHTML;
  const arrow = trigger.querySelector("svg").parentElement;
  assert.ok(arrow.classList.contains("transition-disclosure"));
  assert.ok(arrow.classList.contains("motion-reduce:transition-none"));
  assert.equal(trigger.getAttribute("aria-expanded"), "false");
  await action(w, () => w.user.tab());
  await action(w, () => w.user.keyboard("{Enter}"));
  assert.equal(trigger.getAttribute("aria-expanded"), "true");
  assert.ok(w.q.getByText("설명을 확인해 주세요", { exact: true }));
  assert.ok(trigger.getAttribute("aria-controls"));
  assert.equal(trigger.querySelector("svg").innerHTML, glyph);
  assert.ok(trigger.querySelector(".rotate-disclosure"));
  await action(w, () => w.user.keyboard(" "));
  assert.equal(trigger.getAttribute("aria-expanded"), "false");
  assert.deepEqual(calls(w), ["open", "closed"]);
  assert.ok(arrow.classList.contains("transition-disclosure"));
});
test("TextField values, exact name, errors, disabled and IME submission guard", async () => {
  const w = fixture.open();
  w.show("TextField", { hint: "도움말" });
  const input = w.q.getByRole("textbox", {
    name: "주식 코파일럿",
    exact: true,
  });
  await action(w, () => w.user.type(input, "한글"));
  assert.equal(input.value, "한글");
  assert.equal(calls(w).at(-1), "한글");
  for (const [composing, code, phase] of [
    [true, 13, "idle"],
    [false, 229, "idle"],
    [false, 13, "composing"],
    [false, 13, "ending"],
  ]) {
    const result = w.commit(composing, code, phase);
    assert.equal(result.result, "ignore");
    assert.equal(result.prevented, true);
  }
  assert.equal(w.commit(false, 13, "idle").result, "commit");
  w.fire.compositionStart(input);
  await action(w, () => w.user.keyboard("{Enter}"));
  assert.deepEqual(Array.from(w.submits), []);
  w.fire.compositionEnd(input);
  await action(w, () => w.user.keyboard("{Enter}"));
  assert.deepEqual(Array.from(w.submits), []);
  await action(w, () => w.user.keyboard("{Enter}"));
  assert.deepEqual(Array.from(w.submits), ["submit"]);
  w.show("TextField", { disabled: "disabled", error: "오류를 고쳐 주세요" });
  const disabled = w.q.getByRole("textbox", {
    name: "주식 코파일럿",
    exact: true,
  });
  assert.equal(disabled.disabled, true);
  assert.equal(disabled.getAttribute("aria-invalid"), "true");
  assert.equal(w.q.getByRole("alert").textContent, "오류를 고쳐 주세요");
  w.fire.change(disabled, { target: { value: "blocked" } });
  assert.deepEqual(calls(w), []);
});
test("Secret input masks again after user/external clearing, sec attributes, controlled value and clipboard failures", async () => {
  const w = fixture.open();
  w.show("TextField", { type: "secret" });
  const input = w.q.getByLabelText("주식 코파일럿", { exact: true });
  assert.equal(input.type, "password");
  assert.equal(input.autocomplete, "off");
  assert.equal(input.getAttribute("spellcheck"), "false");
  assert.equal(input.getAttribute("autocapitalize"), "none");
  assert.equal(w.q.queryByRole("button", { name: "보기", exact: true }), null);
  w.clipboard("ok");
  await action(w, () =>
    w.user.click(w.q.getByRole("button", { name: "붙여넣기", exact: true })),
  );
  assert.equal(input.value, "example-secret");
  assert.equal(input.getAttribute("value"), "example-secret");
  await action(w, () =>
    w.user.click(w.q.getByRole("button", { name: "보기", exact: true })),
  );
  assert.equal(input.type, "text");
  await action(w, () => w.user.clear(input));
  assert.equal(input.type, "password");
  await action(w, () => w.user.type(input, "next"));
  assert.equal(input.type, "password");
  await action(w, () =>
    w.user.click(w.q.getByRole("button", { name: "보기", exact: true })),
  );
  await action(w, () => w.setField(""));
  assert.equal(input.type, "password");
  await action(w, () => w.setField("next"));
  assert.equal(input.type, "password");
  for (const mode of ["deny", "absent"]) {
    w.clipboard(mode);
    await action(w, () =>
      w.user.click(w.q.getByRole("button", { name: "붙여넣기", exact: true })),
    );
    assert.ok(w.q.getByRole("alert"));
    assert.equal(input.value, "next");
  }
  w.show("TextField", { type: "secret", disabled: "disabled" });
  w.clipboard("ok");
  w.fire.click(w.q.getByRole("button", { name: "붙여넣기", exact: true }));
  assert.deepEqual(calls(w), []);
});
test("Native Select keeps its exact label apart from options, hint and error", async () => {
  const w = fixture.open();
  w.show("Select", { hint: "도움말" });
  const select = w.q.getByRole("combobox", {
    name: "주식 코파일럿",
    exact: true,
  });
  assert.equal(select.tagName, "SELECT");
  await action(w, () => w.user.tab());
  assert.equal(w.document.activeElement, select);
  await action(w, () => w.user.keyboard("{ArrowDown}{Enter}"));
  await action(w, () => w.user.selectOptions(select, "month"));
  assert.deepEqual(calls(w), ["month"]);
  assert.equal(select.value, "month");
  w.show("Select", { disabled: "disabled", error: "오류를 고쳐 주세요" });
  const disabled = w.q.getByRole("combobox", {
    name: "주식 코파일럿",
    exact: true,
  });
  assert.equal(disabled.disabled, true);
  assert.equal(disabled.getAttribute("aria-invalid"), "true");
  w.fire.change(disabled, { target: { value: "year" } });
  assert.deepEqual(calls(w), []);
  assert.equal(w.q.getByRole("alert").textContent, "오류를 고쳐 주세요");
});
test("SegmentedControl arrows move focus, Space selects and cannot empty selection", async () => {
  const w = fixture.open();
  w.show("SegmentedControl");
  const buttons = w.q.getAllByRole("button");
  assert.equal(buttons[0].getAttribute("aria-pressed"), "true");
  await action(w, () => w.user.tab());
  await action(w, () => w.user.keyboard("{ArrowRight} "));
  assert.equal(w.document.activeElement, buttons[1]);
  assert.equal(buttons[1].getAttribute("aria-pressed"), "true");
  assert.equal(buttons[0].getAttribute("aria-pressed"), "false");
  assert.deepEqual(calls(w), ["month"]);
  await action(w, () => w.user.keyboard(" "));
  assert.equal(buttons[1].getAttribute("aria-pressed"), "true");
});
test("Heading, Text and Icon preserve roles, exact names and decorative states", () => {
  const w = fixture.open();
  for (const level of [1, 2, 3]) {
    w.show("Heading", { level });
    assert.equal(
      w.q.getByRole("heading", { level, name: "주식 코파일럿", exact: true })
        .textContent,
      "주식 코파일럿",
    );
  }
  for (const variant of ["body", "body-sm", "label", "caption", "code"])
    for (const tone of ["ink", "ink-muted", "ink-subtle", "primary", "alert"]) {
      w.show("Text", { variant, tone });
      assert.ok(w.q.getByText("주식 코파일럿", { exact: true }));
    }
  for (const as of ["p", "span", "div"]) {
    w.show("Text", { as });
    assert.equal(
      w.q.getByText("주식 코파일럿", { exact: true }).tagName,
      as.toUpperCase(),
    );
  }
  w.show("Icon", { name: "search", label: "주식 코파일럿" });
  assert.ok(w.q.getByRole("img", { name: "주식 코파일럿", exact: true }));
  w.show("Icon", { name: "search" });
  assert.equal(w.q.queryByRole("img"), null);
  assert.equal(
    w.document.querySelector("svg").getAttribute("aria-hidden"),
    "true",
  );
});
test("All 393 separately registered stories render their DOM state in both themes", () => {
  const w = fixture.open();
  assert.deepEqual(
    [...new Set(Array.from(w.stories, (s) => s.component))].sort(),
    [
      "Heading",
      "Text",
      "Button",
      "IconButton",
      "TextField",
      "Select",
      "Checkbox",
      "Switch",
      "SegmentedControl",
      "Disclosure",
      "Icon",
    ].sort(),
  );
  for (const story of w.stories)
    assert.deepEqual(Array.from(story.themes), ["light", "dark"]);
  for (const theme of ["light", "dark"]) {
    const states = w.renderStories(theme);
    assert.equal(states.length, 393);
    for (const state of states) {
      assert.equal(state.rendered, true, JSON.stringify(state));
      if (state.state === "disabled")
        assert.equal(state.disabled, true, JSON.stringify(state));
      if (state.state === "selected")
        assert.equal(state.selected, true, JSON.stringify(state));
      if (state.state === "open")
        assert.equal(state.open, true, JSON.stringify(state));
      if (state.state === "error") {
        assert.equal(state.invalid, "true");
        assert.equal(state.alert, true);
      }
    }
  }
});
