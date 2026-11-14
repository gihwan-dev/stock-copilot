import assert from "node:assert/strict";
import { before, after, test } from "node:test";
import { componentsDom } from "./ds-components-dom.mjs";
let fixture;
before(async () => {
  fixture = await componentsDom(String.raw`
import {createElement as h,useState} from "react";
import {render,cleanup,act,fireEvent,queries,configure} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {Card,Table,Dialog,EmptyState,Skeleton,Text,Button,TextField} from DS_ENTRY;
import {text} from LIB_ENTRY;
import {feedbackComponentStories} from STORY_ENTRY;
configure({asyncWrapper:(callback)=>callback()});
const label=text("app.title","ko"), description=text("ds.description","ko");
window.cleanup=cleanup;window.act=act;window.fire=fireEvent;
window.q=Object.fromEntries(Object.entries(queries).map(([name,query])=>[name,(...args)=>query(document.body,...args)]));
window.user=userEvent.setup({document});
window.stories=feedbackComponentStories;
window.renderStory=(index,theme)=>{cleanup();document.documentElement.dataset.theme=theme;render(feedbackComponentStories[index].render());};
window.makeAction=(disabled="enabled")=>h(Button,{disabled,onClick:()=>window.calls.push("action")},description);
window.show=(name,props={})=>{cleanup();window.calls=[];
if(name==="Card")return render(h(Card,props,h(Text,null,description)));
if(name==="EmptyState")return render(h(EmptyState,{title:label,...props}));
if(name==="Skeleton")return render(h(Skeleton,props));
if(name==="TableEmpty")return render(h(Table.Root,props));
if(name==="Table")return render(h(Table.Root,{caption:label,...props},h(Table.Head,null,h(Table.Row,null,h(Table.Header,null,label),h(Table.Header,{align:"end"},description))),h(Table.Body,null,h(Table.Row,null,h(Table.RowHeader,null,label),h(Table.Cell,{align:"end"},h(Text,null,description))))));
const Host=()=>{const[open,setOpen]=useState("closed");return h("div",null,h(Button,{onClick:()=>setOpen("open")},label),h(Dialog,{open,title:label,onClose:(reason)=>{window.calls.push(reason);setOpen("closed");},...props},h(TextField,{label:description})));};return render(h(Host));};
`);
});
after(() => fixture?.close());
const action = async (w, fn) => w.act(fn);
test("DS04 Card supports absent and present header and actions", () => {
  const w = fixture.open();
  w.show("Card");
  assert.equal(w.q.queryByRole("heading"), null);
  w.show("Card", { title: "주식 코파일럿" });
  assert.equal(
    w.q.getByRole("heading", { name: "주식 코파일럿", exact: true })
      .textContent,
    "주식 코파일럿",
  );
});
test("DS04 Table preserves row and column headers and numeric alignment in both densities", () => {
  const w = fixture.open();
  for (const dense of ["normal", "dense"]) {
    w.show("Table", { dense });
    assert.equal(
      w.q
        .getByRole("table", { name: "주식 코파일럿", exact: true })
        .classList.contains("tabular-nums"),
      true,
    );
    assert.equal(w.q.getAllByRole("columnheader").length, 2);
    assert.equal(w.q.getByRole("rowheader").getAttribute("scope"), "row");
    const cell = w.q.getByRole("cell");
    assert.equal(cell.classList.contains("text-right"), true);
    assert.equal(
      cell.classList.contains(
        dense === "dense" ? "h-row-height-dense" : "h-row-height",
      ),
      true,
    );
  }
});
test("DS04 EmptyState supports title only and explanatory text", () => {
  const w = fixture.open();
  w.show("EmptyState");
  assert.equal(
    w.q.getByText("주식 코파일럿", { exact: true }).textContent,
    "주식 코파일럿",
  );
  assert.equal(w.q.queryByRole("alert"), null);
  w.show("EmptyState", { children: "설명을 확인해 주세요", icon: "wallet" });
  assert.equal(
    w.q.getByText("설명을 확인해 주세요", { exact: true }).textContent,
    "설명을 확인해 주세요",
  );
});
test("DS04 Skeleton is hidden and static for text and every block height", () => {
  const w = fixture.open();
  for (const props of [
    {},
    { lines: 6 },
    ...["chart-height-sm", "chart-height", "chart-height-lg"].map((height) => ({
      variant: "block",
      height,
    })),
  ]) {
    w.show("Skeleton", props);
    const placeholder = w.document.querySelector('[aria-hidden="true"]');
    assert.ok(placeholder);
    assert.equal(
      placeholder.childElementCount,
      props.variant === "block" ? 0 : (props.lines ?? 3),
    );
    assert.equal(w.document.querySelector('[class*="animate"],[style]'), null);
  }
});
for (const wide of ["normal", "wide"]) {
  test(`DS04 Dialog ${wide} exact name, first input, trapped focus, Escape and return focus`, async () => {
    const w = fixture.open();
    w.show("Dialog", { wide });
    assert.equal(w.q.queryByRole("dialog"), null);
    const trigger = w.q.getByRole("button", {
      name: "주식 코파일럿",
      exact: true,
    });
    await action(w, () => w.user.click(trigger));
    const dialog = w.q.getByRole("dialog", {
      name: "주식 코파일럿",
      exact: true,
    });
    assert.equal(dialog.getAttribute("aria-modal"), "true");
    assert.equal(
      w.document.activeElement,
      w.q.getByRole("textbox", { name: "설명을 확인해 주세요", exact: true }),
    );
    await action(w, () => w.user.tab({ shift: true }));
    assert.equal(dialog.contains(w.document.activeElement), true);
    await action(w, () => w.user.keyboard("{Escape}"));
    assert.deepEqual(Array.from(w.calls), ["escape"]);
    assert.equal(w.q.queryByRole("dialog"), null);
    assert.equal(w.document.activeElement, trigger);
    await action(w, () => w.user.keyboard("{Escape}"));
    assert.deepEqual(Array.from(w.calls), ["escape"]);
  });
}
test("DS04 Dialog close button and background emit only closed reasons", async () => {
  const w = fixture.open();
  for (const reason of ["close-button", "outside"]) {
    w.show("Dialog");
    await action(w, () =>
      w.user.click(
        w.q.getByRole("button", { name: "주식 코파일럿", exact: true }),
      ),
    );
    if (reason === "close-button")
      await action(w, () =>
        w.user.click(w.q.getByRole("button", { name: "닫기", exact: true })),
      );
    else
      await action(w, () =>
        w.user.click(w.document.querySelector("[data-dialog-backdrop]")),
      );
    assert.deepEqual(Array.from(w.calls), [reason]);
    assert.equal(w.q.queryByRole("dialog"), null);
  }
});

test("DS04 every registered story renders its state in both themes", async () => {
  const w = fixture.open();
  assert.equal(w.stories.length, 20);
  assert.deepEqual(
    Object.fromEntries(
      ["Card", "Table", "Dialog", "EmptyState", "Skeleton"].map((name) => [
        name,
        Array.from(w.stories).filter((s) => s.component === name).length,
      ]),
    ),
    { Card: 4, Table: 3, Dialog: 4, EmptyState: 4, Skeleton: 5 },
  );
  for (const [index, story] of Array.from(w.stories).entries()) {
    assert.deepEqual(Array.from(story.themes), ["light", "dark"]);
    for (const theme of story.themes) {
      await action(w, () => w.renderStory(index, theme));
      if (story.component === "Dialog")
        assert.equal(
          w.q.queryByRole("dialog", { name: "주식 코파일럿", exact: true }) !==
            null,
          story.state === "open",
        );
      else assert.ok(w.document.body.firstElementChild.firstElementChild);
    }
  }
});

test("DS04 optional actions preserve enabled and disabled child behavior", async () => {
  const w = fixture.open();
  for (const component of ["Card", "EmptyState"]) {
    for (const disabled of ["enabled", "disabled"]) {
      const key = component === "Card" ? "actions" : "action";
      w.show(component, { [key]: w.makeAction(disabled) });
      await action(w, () =>
        w.user.click(
          w.q.getByRole("button", {
            name: "설명을 확인해 주세요",
            exact: true,
          }),
        ),
      );
      assert.deepEqual(
        Array.from(w.calls),
        disabled === "enabled" ? ["action"] : [],
      );
    }
  }
});
test("DS04 empty Table shows its default or supplied message", () => {
  const w = fixture.open();
  w.show("TableEmpty");
  assert.equal(w.q.getByRole("cell").textContent, "표시할 항목이 없어요");
  w.show("TableEmpty", { empty: "설명을 확인해 주세요" });
  assert.equal(w.q.getByRole("cell").textContent, "설명을 확인해 주세요");
});
test("DS04 clicking dialog content preserves the open state", async () => {
  const w = fixture.open();
  w.show("Dialog");
  await action(w, () =>
    w.user.click(
      w.q.getByRole("button", { name: "주식 코파일럿", exact: true }),
    ),
  );
  await action(w, () =>
    w.user.click(
      w.q.getByRole("textbox", { name: "설명을 확인해 주세요", exact: true }),
    ),
  );
  assert.deepEqual(Array.from(w.calls), []);
  assert.ok(w.q.getByRole("dialog", { name: "주식 코파일럿", exact: true }));
});
