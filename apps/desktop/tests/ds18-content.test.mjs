import assert from "node:assert/strict";
import { before, after, test } from "node:test";
import { componentsDom } from "./ds-components-dom.mjs";
let fixture;
before(async () => {
  fixture = await componentsDom(String.raw`
import {createElement as h} from "react";
import {render,cleanup,act,fireEvent} from "@testing-library/react";
import {Markdown,ChatInput} from DS_ENTRY;
import {parseFreeText} from LIB_ENTRY;
import {contentComponentStories} from STORY_ENTRY;
window.cleanup=cleanup;window.act=act;window.fire=fireEvent;
window.markdown=(value,allowed="allow")=>{cleanup();window.calls=[];window.markdownView=render(h(Markdown,{content:parseFreeText(value).value,linkAllowed:()=>allowed,onOpenLink:url=>window.calls.push(url)}));};
window.updateMarkdown=value=>window.markdownView.rerender(h(Markdown,{content:parseFreeText(value).value,linkAllowed:()=>"allow",onOpenLink:url=>window.calls.push(url)}));
window.chat=(status="ready")=>{cleanup();window.calls=[];window.chatView=render(h(ChatInput,{status,onSend:value=>window.calls.push(value)}));};
window.setStatus=(status)=>window.chatView.rerender(h(ChatInput,{status,onSend:value=>window.calls.push(value)}));
window.stories=contentComponentStories;
window.story=(index,theme)=>{cleanup();document.documentElement.dataset.theme=theme;render(contentComponentStories[index].render());};
window.input=()=>document.querySelector("textarea");window.send=()=>document.querySelector("button");
`);
});
after(() => fixture?.close());
test("DS18 markdown excludes HTML and remote images, uses callback only", async () => {
  const w = fixture.open();
  await w.act(() =>
    w.markdown(
      '<script>alert(1)</script>\n\n<img src="https://evil.test/pixel">\n\n![사진](https://example.test/image)\n\n[출처](https://example.test/article)',
    ),
  );
  assert.equal(
    w.document.querySelectorAll("script,img,a,[href],[src]").length,
    0,
  );
  assert.equal(w.document.body.textContent.includes("alert(1)"), false);
  assert.equal(w.document.body.textContent.includes("사진"), true);
  const buttons = w.document.querySelectorAll("button");
  assert.equal(buttons.length, 2);
  await w.act(() => w.fire.click(buttons[1]));
  assert.deepEqual(Array.from(w.calls), ["https://example.test/article"]);
  await w.act(() => w.fire.click(buttons[0]));
  assert.deepEqual(Array.from(w.calls), [
    "https://example.test/article",
    "https://example.test/image",
  ]);
});
test("DS18 denied and unsafe addresses stay readable without actions", async () => {
  const w = fixture.open();
  for (const url of [
    "javascript:alert%281%29",
    "data:text/plain,hello",
    "http://example.test",
    "//example.test",
    "/local",
    "mailto:a@example.test",
  ]) {
    await w.act(() => w.markdown(`[주소](${url})`));
    assert.equal(w.document.querySelectorAll("button,a,img").length, 0);
    assert.equal(w.document.body.textContent, "주소");
  }
  await w.act(() => w.markdown("[거부](https://example.test)", "deny"));
  assert.equal(w.document.querySelectorAll("button,a").length, 0);
  assert.equal(w.document.body.textContent, "거부");
  await w.act(() => w.markdown("<https://example.test>"));
  assert.equal(w.document.querySelectorAll("a").length, 0);
  assert.equal(w.document.querySelectorAll("button").length, 1);
});
test("DS18 code, footnotes and table syntax remain readable", async () => {
  const w = fixture.open();
  const value =
    "긴글".repeat(1000) +
    "\n\n```js\n" +
    "word".repeat(500) +
    "\n```\n\n| 이름 | 값 |\n| --- | --- |\n| 예시 | 1 |\n\n설명[^1]\n\n[^1]: 각주";
  await w.act(() => w.markdown(value));
  assert.ok(w.document.querySelector("pre"));
  assert.equal(w.document.querySelectorAll("table").length, 0);
  assert.ok(w.document.body.textContent.includes("설명"));
});
test("DS18 chat rejects blanks and sending, sends original text once", async () => {
  const w = fixture.open();
  for (const value of ["", " \n\t", "질문입니다"]) {
    await w.act(() => w.chat());
    await w.act(() => w.fire.change(w.input(), { target: { value } }));
    await w.act(() => w.fire.keyDown(w.input(), { key: "Enter" }));
    assert.deepEqual(Array.from(w.calls), value.trim() ? [value] : []);
    assert.equal(w.input().value, value.trim() ? "" : value);
  }
  await w.act(() => w.chat());
  await w.act(() =>
    w.fire.change(w.input(), { target: { value: "  원문\n유지  " } }),
  );
  await w.act(() => {
    w.fire.click(w.send());
  });
  await w.act(() => w.fire.click(w.send()));
  assert.deepEqual(Array.from(w.calls), ["  원문\n유지  "]);
  await w.act(() => w.chat("sending"));
  assert.equal(w.input().disabled, false);
  assert.equal(w.input().readOnly, true);
  assert.equal(w.send().disabled, true);
  await w.act(() => {
    w.fire.change(w.input(), { target: { value: "차단" } });
    w.fire.keyDown(w.input(), { key: "Enter" });
    w.fire.click(w.send());
  });
  assert.equal(w.calls.length, 0);
});
test("DS18 IME and Shift Enter never send", async () => {
  const w = fixture.open();
  for (const event of [
    { key: "Enter", isComposing: true },
    { key: "Enter", keyCode: 229 },
    { key: "Enter", shiftKey: true },
    { key: "a" },
  ]) {
    await w.act(() => w.chat());
    await w.act(() => w.fire.change(w.input(), { target: { value: "한글" } }));
    await w.act(() => w.fire.keyDown(w.input(), event));
    assert.equal(w.calls.length, 0);
  }
  await w.act(() => w.chat());
  await w.act(() => w.fire.change(w.input(), { target: { value: "한글" } }));
  await w.act(() => w.fire.compositionStart(w.input()));
  await w.act(() => w.fire.keyDown(w.input(), { key: "Enter" }));
  assert.equal(w.calls.length, 0);
  await w.act(() => w.fire.compositionEnd(w.input()));
  await w.act(() => w.fire.keyDown(w.input(), { key: "Enter" }));
  assert.equal(w.calls.length, 0);
  await w.act(() => w.fire.keyDown(w.input(), { key: "Enter" }));
  assert.deepEqual(Array.from(w.calls), ["한글"]);
});

test("DS18 pending draft blocks send and edits while sending", async () => {
  const w = fixture.open();
  await w.act(() => w.chat());
  await w.act(() => w.fire.change(w.input(), { target: { value: "원문" } }));
  await w.act(() => w.setStatus("sending"));
  await w.act(() => w.fire.keyDown(w.input(), { key: "Enter" }));
  assert.equal(w.calls.length, 0);
  await w.act(() => w.fire.change(w.input(), { target: { value: "변경" } }));
  await w.act(() => w.setStatus("ready"));
  assert.equal(w.input().value, "원문");
  await w.act(() => w.fire.compositionStart(w.input()));
  await w.act(() => w.fire.click(w.send()));
  assert.equal(w.calls.length, 0);
});

test("DS18 completed composition permits one button send", async () => {
  const w = fixture.open();
  await w.act(() => w.chat());
  await w.act(() => w.fire.compositionStart(w.input()));
  await w.act(() => w.fire.change(w.input(), { target: { value: "안녕" } }));
  await w.act(() => w.fire.compositionEnd(w.input()));
  await w.act(() => w.fire.blur(w.input()));
  await w.act(() => w.fire.click(w.send()));
  await w.act(() => w.fire.click(w.send()));
  assert.deepEqual(Array.from(w.calls), ["안녕"]);
});
test("DS18 image inside a link has one interactive target", async () => {
  const w = fixture.open();
  for (const permission of ["allow", "deny"]) {
    await w.act(() =>
      w.markdown("[![그림](https://a.test/i)](https://b.test/p)", permission),
    );
    assert.equal(w.document.querySelectorAll("button button,img,a").length, 0);
    const buttons = w.document.querySelectorAll("button");
    assert.equal(buttons.length, permission === "allow" ? 1 : 0);
    if (buttons.length) await w.act(() => w.fire.click(buttons[0]));
    assert.deepEqual(
      Array.from(w.calls),
      permission === "allow" ? ["https://b.test/p"] : [],
    );
  }
});
test("DS18 ending Enter is consumed while Shift Enter remains native", async () => {
  const w = fixture.open();
  await w.act(() => w.chat());
  await w.act(() => w.fire.compositionStart(w.input()));
  await w.act(() => w.fire.compositionEnd(w.input()));
  let native;
  await w.act(() => {
    native = w.fire.keyDown(w.input(), { key: "Enter" });
  });
  assert.equal(native, false);
  await w.act(() => {
    native = w.fire.keyDown(w.input(), { key: "Enter", shiftKey: true });
  });
  assert.equal(native, true);
});
test("DS18 stories render both themes with mixed markdown and long draft", async () => {
  const w = fixture.open();
  assert.equal(w.stories.length, 6);
  for (let i = 0; i < w.stories.length; i++)
    for (const theme of ["light", "dark"]) {
      await w.act(() => w.story(i, theme));
      const story = w.stories[i];
      if (story.draft !== undefined)
        await w.act(() =>
          w.fire.change(w.input(), { target: { value: story.draft } }),
        );
      assert.deepEqual(Array.from(story.themes), ["light", "dark"]);
      if (story.component === "Markdown" && story.state === "stress") {
        assert.equal(w.document.querySelectorAll("h2").length, 100);
        assert.equal(w.document.querySelectorAll("pre").length, 100);
        assert.equal(w.document.querySelectorAll("button").length, 100);
      }
      if (story.component === "ChatInput") {
        assert.ok(w.input().classList.contains("chat-input"));
        if (story.state === "stress") assert.ok(w.input().value.length > 1000);
      }
    }
});

test("DS18 sending keeps the draft focusable and read only", async () => {
  const w = fixture.open();
  await w.act(() => w.chat());
  w.input().focus();
  await w.act(() => w.setStatus("sending"));
  assert.equal(w.document.activeElement === w.input(), true);
  assert.equal(w.input().readOnly, true);
  await w.act(() => w.setStatus("ready"));
  assert.equal(w.input().readOnly, false);
  assert.equal(w.document.activeElement === w.input(), true);
});

test("DS18 old composition cannot consume Enter after focus or send", async () => {
  const w = fixture.open();
  await w.act(() => w.chat());
  await w.act(() => w.fire.compositionStart(w.input()));
  await w.act(() => w.fire.change(w.input(), { target: { value: "안녕" } }));
  await w.act(() => w.fire.compositionEnd(w.input()));
  await w.act(() => w.fire.blur(w.input()));
  await w.act(() => w.fire.focus(w.input()));
  await w.act(() => w.fire.keyDown(w.input(), { key: "Enter" }));
  assert.deepEqual(Array.from(w.calls), ["안녕"]);
  await w.act(() => w.fire.compositionStart(w.input()));
  await w.act(() => w.fire.change(w.input(), { target: { value: "두 번째" } }));
  await w.act(() => w.fire.compositionEnd(w.input()));
  await w.act(() => w.fire.click(w.send()));
  await w.act(() =>
    w.fire.change(w.input(), { target: { value: "붙여넣기" } }),
  );
  await w.act(() => w.fire.keyDown(w.input(), { key: "Enter" }));
  assert.deepEqual(Array.from(w.calls), ["안녕", "두 번째", "붙여넣기"]);
});
test("DS18 streaming preserves link and code nodes and focus", async () => {
  const w = fixture.open();
  const initial = "[출처](https://a.test)\n\n```text\n원문\n```";
  await w.act(() => w.markdown(initial));
  const button = w.document.querySelector("button"),
    pre = w.document.querySelector("pre");
  button.focus();
  await w.act(() => w.updateMarkdown(initial + "\n\n추가 문단"));
  assert.equal(w.document.querySelector("button") === button, true);
  assert.equal(w.document.querySelector("pre") === pre, true);
  assert.equal(w.document.activeElement === button, true);
});
test("DS18 empty link names never create unnamed tab targets", async () => {
  const w = fixture.open();
  for (const value of [
    "[](https://a.test)",
    "[ ](https://a.test)",
    "[![]()](https://a.test)",
  ]) {
    await w.act(() => w.markdown(value));
    assert.equal(w.document.querySelectorAll("button,a").length, 0);
  }
});
test("DS18 stress story renders a multiline draft without metadata injection", async () => {
  const w = fixture.open();
  for (const theme of ["light", "dark"]) {
    const i = Array.from(w.stories).findIndex(
      (s) => s.component === "ChatInput" && s.state === "stress",
    );
    await w.act(() => w.story(i, theme));
    assert.ok(w.input().value.length > 1000);
    assert.ok(w.input().value.includes("\n"));
  }
});

test("DS18 semantic nodes retain hierarchy and nondefault list start", async () => {
  const w = fixture.open();
  await w.act(() =>
    w.markdown(
      "# 하나\n\n## 둘\n\n### 셋\n\n#### 넷\n\n##### 다섯\n\n###### 여섯\n\n3. 순서\n4. 다음\n\n- 항목\n\n> 인용\n\n문단 `코드`",
    ),
  );
  for (const [tag, token] of [
    ["h1", "text-title-lg"],
    ["h2", "text-title-md"],
    ["h3", "text-title-sm"],
    ["h4", "text-title-sm"],
    ["h5", "text-title-sm"],
    ["h6", "text-title-sm"],
    ["ol", "markdown-ordered"],
    ["ul", "markdown-bullets"],
    ["blockquote", "markdown-quote"],
    ["code", "font-mono"],
  ])
    assert.ok(w.document.querySelector(tag).classList.contains(token));
  assert.equal(w.document.querySelector("ol").getAttribute("start"), "3");
  assert.ok(w.document.querySelector("p").classList.contains("my-space-3"));
});

test("DS18 blank image alt uses a readable URL in standalone and nested links", async () => {
  const w = fixture.open();
  for (const alt of [" ", "&nbsp;", "\u200b"]) {
    await w.act(() => w.markdown(`![${alt}](https://i.test/image)`));
    let buttons = w.document.querySelectorAll("button");
    assert.equal(buttons.length, 1);
    assert.equal(buttons[0].textContent, "https://i.test/image");
    await w.act(() => w.fire.click(buttons[0]));
    assert.deepEqual(Array.from(w.calls), ["https://i.test/image"]);
    await w.act(() =>
      w.markdown(`[![${alt}](https://i.test/image)](https://a.test/article)`),
    );
    buttons = w.document.querySelectorAll("button");
    assert.equal(buttons.length, 1);
    assert.equal(buttons[0].textContent, "https://i.test/image");
    assert.equal(w.document.querySelectorAll("button button").length, 0);
    await w.act(() => w.fire.click(buttons[0]));
    assert.deepEqual(Array.from(w.calls), ["https://a.test/article"]);
    await w.act(() => w.markdown(`![${alt}]()`));
    assert.equal(w.document.querySelectorAll("button").length, 0);
  }
});
test("DS18 invisible link text cannot create a tab target", async () => {
  const w = fixture.open();
  for (const label of ["\u200b", "\u2060", "\u00ad", "\u200b&nbsp;"]) {
    await w.act(() => w.markdown(`[${label}](https://a.test)`));
    assert.equal(w.document.querySelectorAll("button,a").length, 0);
  }
  await w.act(() => w.markdown("[보이는 글\u200b](https://a.test)"));
  assert.equal(w.document.querySelectorAll("button").length, 1);
});
