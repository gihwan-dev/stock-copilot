import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { repoRoot } from "./paths.mjs";
import { join } from "node:path";
import { before, after, test } from "node:test";
import { componentsBrowser } from "./ds-components-browser.mjs";
const designTokens = JSON.parse(
  readFileSync(join(repoRoot, "design/tokens.json"), "utf8"),
);
const color = (name, theme) => {
  const hex = designTokens.color.tokens.find((t) => t.name === name).value[
    theme
  ];
  return (
    "rgb(" +
    [1, 3, 5]
      .map((start) => parseInt(hex.slice(start, start + 2), 16))
      .join(", ") +
    ")"
  );
};
let fixture;
before(async () => {
  fixture = await componentsBrowser(String.raw`
import {createElement as h,useState} from "react";
import {render,cleanup} from "@testing-library/react";
import {Button,IconButton,Checkbox,Switch,Disclosure,TextField,Select,SegmentedControl,Text,Heading,baseComponentStories} from DS_ENTRY;
import {text,parseInputValue,emptyInputValue} from LIB_ENTRY;
const label=text("app.title","ko");
const value=(raw)=>{const parsed=parseInputValue(raw);if(parsed.kind!=="ok") throw Error("invalid fixture");return parsed.value;};
const options=[{value:value("day"),label:text("ds.day","ko")},{value:value("month"),label:text("ds.month","ko")},{value:value("year"),label:text("ds.year","ko")}];
window.show=(name,props={})=>{
cleanup();window.calls=[];window.commits=[];
const Host=()=>{
const [checked,setChecked]=useState(name==="Switch"?"off":"unchecked");
const [open,setOpen]=useState("closed");
const [field,setField]=useState(emptyInputValue);
const [selected,setSelected]=useState(options[0].value);
const click=()=>window.calls.push("clicked");
const change=(next)=>{window.calls.push(next);setChecked(next);};
const changeField=(next)=>{window.calls.push(next);setField(next);};
const choose=(next)=>{window.calls.push(next);setSelected(next);};
if(name==="Button") return h(Button,{onClick:click,...props},label);
if(name==="IconButton") return h(IconButton,{icon:"settings",label,onClick:click,...props});
if(name==="Checkbox"||name==="Switch") return h(name==="Checkbox"?Checkbox:Switch,{label,description:text("ds.description","ko"),checked,onChange:change,...props});
if(name==="Disclosure") return h(Disclosure,{title:label,open,onOpenChange:(next)=>{window.calls.push(next);setOpen(next);},...props},h(Text,null,text("ds.description","ko")));
if(name==="TextField") return h(TextField,{label,value:field,onChange:changeField,onCommit:(next)=>window.commits.push(next),...props});
if(name==="Select") return h(Select,{label,options,value:selected,onChange:choose,...props});
if(name==="SegmentedControl") return h(SegmentedControl,{label,options,value:selected,onChange:choose,...props});
if(name==="Heading") return h(Heading,props,label);
return h(Text,props,label);
};render(h(Host));
};
window.stories=()=>{cleanup();return baseComponentStories.map(story=>({component:story.component,state:story.state,themes:story.themes}));};
window.renderStories=(theme)=>{cleanup();document.documentElement.dataset.theme=theme;const view=render(h("div",null,...baseComponentStories.map((story,index)=>h("div",{"data-testid":"story-"+index,key:index},story.render()))));return baseComponentStories.map((story,index)=>{const node=view.getByTestId("story-"+index);const input=node.querySelector("input:not([type=hidden]),select");return {component:story.component,state:story.state,rendered:node.childElementCount>0,disabled:node.querySelector(":disabled")!==null,invalid:input?.getAttribute("aria-invalid"),alert:node.querySelector('[role="alert"]')!==null,selected:node.querySelector('[aria-checked="true"],[aria-pressed="true"]')!==null,open:node.querySelector('[aria-expanded="true"]')!==null};});};
window.composition=(type)=>document.querySelector("input").dispatchEvent(new CompositionEvent(type,{bubbles:true,data:"한"}));
window.ime=(composing,code=13)=>{const input=document.querySelector("input");input.dispatchEvent(new KeyboardEvent("keydown",{key:"Enter",keyCode:code,isComposing:composing,bubbles:true}));};
window.clipboard=(mode)=>Object.defineProperty(navigator,"clipboard",{configurable:true,value:mode==="absent"?undefined:{readText:()=>mode==="deny"?Promise.reject(Error("denied")):Promise.resolve("  example-secret \n")}});
`);
});
after(async () => {
  await fixture?.close();
});
const show = (page, name, props = {}) =>
  page.evaluate(
    "window.show(" + JSON.stringify(name) + "," + JSON.stringify(props) + ")",
  );
const calls = (page) => page.evaluate("window.calls");

test("Button variants, sizes, hover, focus and disabled keyboard contract", async () => {
  const page = await fixture.page();
  try {
    for (const theme of ["light", "dark"]) {
      await page.evaluate(
        "document.documentElement.dataset.theme=" + JSON.stringify(theme),
      );
      for (const variant of ["primary", "secondary", "ghost", "buy", "sell"]) {
        for (const size of ["control-height", "control-height-sm"]) {
          await page.mouse.move(1200, 700);
          await show(page, "Button", { variant, size });
          const button = page.getByRole("button", { name: "주식 코파일럿" });
          assert.equal(
            await button.evaluate(
              (node) =>
                node.ownerDocument.defaultView.getComputedStyle(node).height,
            ),
            size === "control-height" ? "32px" : "28px",
          );
          const base = {
            primary: "primary",
            secondary: "surface",
            buy: "up",
            sell: "down",
          }[variant];
          assert.equal(
            await button.evaluate(
              (node) =>
                node.ownerDocument.defaultView.getComputedStyle(node)
                  .backgroundColor,
            ),
            base === undefined ? "rgba(0, 0, 0, 0)" : color(base, theme),
          );
          await button.hover();
          const hover = {
            primary: "primary-hover",
            secondary: "surface-sunken",
            ghost: "surface-sunken",
            buy: "up-hover",
            sell: "down-hover",
          }[variant];
          assert.equal(
            await button.evaluate(
              (node) =>
                node.ownerDocument.defaultView.getComputedStyle(node)
                  .backgroundColor,
            ),
            color(hover, theme),
          );
          await button.focus();
          await page.keyboard.press("Enter");
          await page.keyboard.press("Space");
          assert.deepEqual(await calls(page), ["clicked", "clicked"]);
          await show(page, "Button", { variant, size, disabled: "disabled" });
          assert.equal(await page.getByRole("button").isDisabled(), true);
          assert.equal(
            await page
              .getByRole("button")
              .evaluate(
                (node) =>
                  node.ownerDocument.defaultView.getComputedStyle(node)
                    .backgroundColor,
              ),
            base === undefined ? "rgba(0, 0, 0, 0)" : color(base, theme),
          );
          await page.getByRole("button").dispatchEvent("click");
          assert.deepEqual(await calls(page), []);
        }
      }
    }
  } finally {
    await page.close();
  }
});

test("IconButton names, pressed, expanded, tooltip and disabled", async () => {
  const page = await fixture.page();
  try {
    await show(page, "IconButton", { pressed: "pressed", expanded: "open" });
    const button = page.getByRole("button", { name: "주식 코파일럿" });
    assert.equal(await button.getAttribute("aria-pressed"), "true");
    assert.equal(await button.getAttribute("aria-expanded"), "true");
    await button.hover();
    await page.getByRole("tooltip").waitFor();
    assert.equal(
      await page.getByRole("tooltip").textContent(),
      "주식 코파일럿",
    );
    await button.focus();
    await page.keyboard.press("Space");
    assert.deepEqual(await calls(page), ["clicked"]);
    await show(page, "IconButton", {
      disabled: "disabled",
      size: "control-height-sm",
      tooltip: "hidden",
    });
    assert.equal(await page.getByRole("button").isDisabled(), true);
    await page.getByRole("button").dispatchEvent("click");
    assert.deepEqual(await calls(page), []);
  } finally {
    await page.close();
  }
});

for (const name of ["Checkbox", "Switch"]) {
  test(`${name} keyboard toggle, controlled state, description and disabled`, async () => {
    const page = await fixture.page();
    try {
      await show(page, name);
      const control = page.getByRole(
        name === "Switch" ? "switch" : "checkbox",
        { name: "주식 코파일럿" },
      );
      assert.equal(await control.getAttribute("aria-checked"), "false");
      await page.keyboard.press("Tab");
      await page.keyboard.press("Space");
      assert.equal(await control.getAttribute("aria-checked"), "true");
      assert.deepEqual(await calls(page), [
        name === "Switch" ? "on" : "checked",
      ]);
      await page.keyboard.press("Space");
      assert.equal(await control.getAttribute("aria-checked"), "false");
      await show(page, name, { disabled: "disabled" });
      await page
        .getByRole(name === "Switch" ? "switch" : "checkbox")
        .dispatchEvent("click");
      assert.deepEqual(await calls(page), []);
    } finally {
      await page.close();
    }
  });
}

test("Disclosure Enter and Space toggle and relate the advanced panel", async () => {
  const page = await fixture.page();
  try {
    await show(page, "Disclosure");
    const trigger = page.getByRole("button", { name: "주식 코파일럿" });
    assert.equal(await trigger.getAttribute("aria-expanded"), "false");
    await page.keyboard.press("Tab");
    await page.keyboard.press("Enter");
    assert.equal(await trigger.getAttribute("aria-expanded"), "true");
    assert.equal(
      await page.getByText("설명을 확인해 주세요").isVisible(),
      true,
    );
    assert.ok(await trigger.getAttribute("aria-controls"));
    await page.keyboard.press("Space");
    assert.equal(await trigger.getAttribute("aria-expanded"), "false");
    assert.deepEqual(await calls(page), ["open", "closed"]);
  } finally {
    await page.close();
  }
});

test("TextField branded input, errors, disabled and Korean IME Enter", async () => {
  const page = await fixture.page();
  try {
    await show(page, "TextField", { hint: "도움말" });
    const input = page.getByRole("textbox", { name: "주식 코파일럿" });
    await input.fill("한글");
    assert.deepEqual(await calls(page), ["한글"]);
    await page.evaluate("window.ime(true)");
    assert.deepEqual(await page.evaluate("window.commits"), []);

    await page.evaluate('window.composition("compositionstart")');
    await page.evaluate("window.ime(false)");
    assert.deepEqual(await page.evaluate("window.commits"), []);
    await page.evaluate('window.composition("compositionend")');
    await page.keyboard.press("Enter");
    assert.deepEqual(await page.evaluate("window.commits"), ["한글"]);
    await show(page, "TextField", {
      error: "오류를 고쳐 주세요",
      disabled: "disabled",
    });
    assert.equal(await page.getByRole("textbox").isDisabled(), true);
    assert.equal(
      await page.getByRole("textbox").getAttribute("aria-invalid"),
      "true",
    );
    assert.equal(
      await page.getByRole("alert").textContent(),
      "오류를 고쳐 주세요",
    );
    assert.deepEqual(await calls(page), []);
  } finally {
    await page.close();
  }
});

test("TextField secret masking, reveal and clipboard success, denial and absence", async () => {
  const page = await fixture.page();
  try {
    await show(page, "TextField", { type: "secret" });
    const input = page.getByLabel("주식 코파일럿", { exact: true });
    assert.equal(await input.getAttribute("type"), "password");
    assert.equal(await input.getAttribute("autocomplete"), "off");
    assert.equal(await input.getAttribute("spellcheck"), "false");
    assert.equal(await page.getByRole("button", { name: "보기" }).count(), 0);
    await page.evaluate('window.clipboard("ok")');
    await page.getByRole("button", { name: "붙여넣기" }).click();
    await page.getByRole("button", { name: "보기" }).waitFor();
    assert.equal(await input.inputValue(), "example-secret");
    await page.getByRole("button", { name: "보기" }).click();
    assert.equal(await input.getAttribute("type"), "text");
    await page.getByRole("button", { name: "가리기" }).click();
    assert.equal(await input.getAttribute("type"), "password");
    await page.evaluate('window.clipboard("deny")');
    await page.getByRole("button", { name: "붙여넣기" }).click();
    await page.getByRole("alert").waitFor();
    assert.equal(await input.inputValue(), "example-secret");
    await page.evaluate('window.clipboard("absent")');
    await page.getByRole("button", { name: "붙여넣기" }).click();
    assert.equal(await input.inputValue(), "example-secret");
  } finally {
    await page.close();
  }
});

test("Select retains the native picker and exposes only selected values", async () => {
  const page = await fixture.page();
  try {
    await show(page, "Select");
    const control = page.getByRole("combobox", { name: "주식 코파일럿" });
    assert.equal(await control.evaluate((node) => node.tagName), "SELECT");
    await control.selectOption("month");
    assert.deepEqual(await calls(page), ["month"]);
    await control.focus();
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Enter");
    assert.equal(await control.isEnabled(), true);
    await control.selectOption("year");
    assert.equal(await control.inputValue(), "year");
    assert.deepEqual(await calls(page), ["month", "year"]);
    await show(page, "Select", {
      disabled: "disabled",
      error: "오류를 고쳐 주세요",
    });
    assert.equal(await page.getByRole("combobox").isDisabled(), true);
    assert.equal(
      await page.getByRole("alert").textContent(),
      "오류를 고쳐 주세요",
    );
    assert.deepEqual(await calls(page), []);
  } finally {
    await page.close();
  }
});

test("SegmentedControl arrows move focus and Space selects a single value", async () => {
  const page = await fixture.page();
  try {
    await show(page, "SegmentedControl");
    const buttons = page.getByRole("button");
    assert.equal(await buttons.nth(0).getAttribute("aria-pressed"), "true");
    await page.keyboard.press("Tab");
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("Space");
    assert.equal(await buttons.nth(1).getAttribute("aria-pressed"), "true");
    assert.equal(await buttons.nth(0).getAttribute("aria-pressed"), "false");
    assert.deepEqual(await calls(page), ["month"]);
    await page.keyboard.press("Space");
    assert.equal(await buttons.nth(1).getAttribute("aria-pressed"), "true");
  } finally {
    await page.close();
  }
});

test("Heading levels and Text type and tone variants retain semantics", async () => {
  const page = await fixture.page();
  try {
    for (const level of [1, 2, 3]) {
      await show(page, "Heading", { level });
      assert.equal(
        await page.getByRole("heading", { level }).textContent(),
        "주식 코파일럿",
      );
    }
    for (const variant of ["body", "body-sm", "label", "caption", "code"]) {
      for (const tone of [
        "ink",
        "ink-muted",
        "ink-subtle",
        "primary",
        "alert",
      ]) {
        await show(page, "Text", { variant, tone });
        assert.equal(
          await page.getByText("주식 코파일럿", { exact: true }).count(),
          1,
        );
      }
    }
    for (const as of ["p", "span", "div"]) {
      await show(page, "Text", { as });
      assert.equal(
        await page.getByText("주식 코파일럿").evaluate((node) => node.tagName),
        as.toUpperCase(),
      );
    }
  } finally {
    await page.close();
  }
});

test("Every public base component has theme-registered state stories", async () => {
  const page = await fixture.page();
  try {
    const stories = await page.evaluate("window.stories()");
    assert.deepEqual(
      [...new Set(stories.map((s) => s.component))].sort(),
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
    for (const story of stories)
      assert.deepEqual(story.themes, ["light", "dark"]);
  } finally {
    await page.close();
  }
});

test("All 403 registered states render with error, disabled, selection and open semantics in both themes", async () => {
  const page = await fixture.page();
  try {
    for (const theme of ["light", "dark"]) {
      const states = await page.evaluate(
        "window.renderStories(" + JSON.stringify(theme) + ")",
      );
      assert.equal(states.length, 403);
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
  } finally {
    await page.close();
  }
});
