import { inspectOverflow } from "../src/shared/ds/testing/overflow";
import { stressFixtures } from "../src/shared/ds/testing/stress";
import { identifyOverflowElement } from "./ds20-overflow-identity.browser";

const makeDocument = async (
  body: string,
  htmlAttributes = "",
): Promise<Document> => {
  const frame = document.createElement("iframe");
  frame.width = "640";
  frame.height = "480";
  const loaded = new Promise<void>((resolve) => {
    frame.addEventListener("load", () => resolve(), { once: true });
  });
  frame.srcdoc = `<html ${htmlAttributes}><head><style>html,body{margin:0;width:100%;height:100%}*{box-sizing:border-box}body{font:16px sans-serif}</style></head><body>${body}</body></html>`;
  document.body.append(frame);
  await loaded;
  if (frame.contentDocument === null)
    throw new Error("iframe document missing");
  return frame.contentDocument;
};

const loadProbeFont = async (doc: Document): Promise<void> => {
  const font = new URL(
    "../../../design/fonts/IBMPlexSansKR-Regular.woff2",
    import.meta.url,
  ).href;
  const style = doc.createElement("style");
  style.textContent = `@font-face{font-family:"IBM Plex Sans KR";src:url("${font}");font-weight:400}`;
  doc.head.append(style);
  const loaded = await doc.fonts.load('400 12px "IBM Plex Sans KR"');
  if (loaded.length === 0) throw new Error("probe font missing");
};

const narrowStory = async (
  component: string,
  exportName: string,
): Promise<{ doc: Document; slot: HTMLElement }> => {
  const response = await fetch("/index.json");
  const index = await response.json();
  const entries = Object.values(index.entries) as {
    id: string;
    importPath: string;
    exportName: string;
  }[];
  const story = entries.find(
    (entry) =>
      entry.importPath.endsWith(`/${component}.stories.mjs`) &&
      entry.exportName === exportName,
  );
  if (story === undefined) throw new Error(`${component} story missing`);
  const frame = document.createElement("iframe");
  frame.width = "960";
  frame.height = "640";
  frame.src = `/iframe.html?id=${encodeURIComponent(story.id)}&viewMode=story`;
  document.body.append(frame);
  const deadline = Date.now() + 10000;
  while (
    frame.contentDocument?.querySelector(
      "[data-catalog-component] > section",
    ) == null
  ) {
    if (Date.now() >= deadline) throw new Error(`${component} did not render`);
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  const doc = frame.contentDocument;
  const view = frame.contentWindow;
  const slot = doc?.querySelector<HTMLElement>(
    "[data-catalog-component] > section",
  );
  if (doc === null || view === null || slot == null)
    throw new Error(`${component} frame missing`);
  await doc.fonts.ready;
  slot.style.boxSizing = "content-box";
  slot.style.width = "126px";
  await new Promise<void>((resolve) =>
    view.requestAnimationFrame(() =>
      view.requestAnimationFrame(() => resolve()),
    ),
  );
  return { doc, slot };
};

afterEach(() => {
  document.querySelectorAll("iframe").forEach((frame) => frame.remove());
});

describe("overflow measurements", () => {
  it.each([
    ["Heading", "Light0", "h1"],
    ["Select", "Light4", "label"],
    ["Select", "Light1", "span.text-caption"],
    ["Select", "Light2", "span[role='alert']"],
  ])(
    "wraps an unbroken %s label in the narrow slot",
    async (component, story, selector) => {
      const { doc, slot } = await narrowStory(component, story);
      const label = slot.querySelector<HTMLElement>(selector);
      if (label === null) throw new Error(`${component} label missing`);
      label.textContent = stressFixtures.english;
      const view = doc.defaultView;
      if (view === null) throw new Error(`${component} view missing`);
      await new Promise<void>((resolve) =>
        view.requestAnimationFrame(() =>
          view.requestAnimationFrame(() => resolve()),
        ),
      );
      expect(label.textContent).toBe(stressFixtures.english);
      expect(label.scrollWidth).toBeLessThanOrEqual(label.clientWidth);
    },
    20000,
  );

  it.each(["LineChart", "AreaChart"])(
    "keeps the full hidden %s table without false clipping verdicts",
    async (component) => {
      const { doc } = await narrowStory(component, "Light4");
      const table = doc.querySelector<HTMLTableElement>("table.sr-only");
      if (table === null || table.parentElement === null)
        throw new Error(`${component} table missing`);
      table.id = "measured-table";
      table.parentElement.id = "measured-wrapper";
      expect(table.querySelectorAll("tr")).toHaveLength(500);
      const findings = inspectOverflow(doc);
      expect(findings).not.toContainEqual({
        kind: "unauthorized-overflow",
        element: "div#measured-wrapper",
      });
      expect(findings).not.toContainEqual({
        kind: "clipped",
        element: "table#measured-table",
      });
    },
    20000,
  );

  it.each([
    ["TextField", "Light6"],
    ["Checkbox", "Light4"],
    ["Switch", "Light4"],
  ])(
    "wraps the %s stress label and description without losing text",
    async (component, story) => {
      const { doc, slot } = await narrowStory(component, story);
      const spans = [...slot.querySelectorAll("span")];
      const label = spans.find(
        (part) => part.textContent === stressFixtures.company,
      );
      const description = spans.find(
        (part) => part.textContent === stressFixtures.english,
      );
      if (label === undefined || description === undefined)
        throw new Error(`${component} stress text missing`);
      const view = doc.defaultView;
      if (view === null) throw new Error(`${component} view missing`);
      for (const scale of ["100%", "125%"] as const) {
        doc.documentElement.style.fontSize = scale;
        await new Promise<void>((resolve) =>
          view.requestAnimationFrame(() =>
            view.requestAnimationFrame(() => resolve()),
          ),
        );
        const slotBounds = slot.getBoundingClientRect();
        for (const part of [label, description]) {
          expect(part.scrollWidth).toBeLessThanOrEqual(part.clientWidth);
          expect(part.getBoundingClientRect().right).toBeLessThanOrEqual(
            slotBounds.right,
          );
          expect(part.getBoundingClientRect().left).toBeGreaterThanOrEqual(
            slotBounds.left,
          );
        }
      }
    },
    20000,
  );

  it("keeps the EmptyState stress text visible in a narrow slot", async () => {
    const { slot } = await narrowStory("EmptyState", "Light5");
    const paragraphs = [...slot.querySelectorAll("p")];
    const title = paragraphs.find(
      (part) => part.textContent === stressFixtures.company,
    );
    const description = paragraphs.find(
      (part) => part.textContent === stressFixtures.english,
    );
    if (title === undefined || description === undefined)
      throw new Error("EmptyState stress text missing");
    expect(title.scrollWidth).toBeLessThanOrEqual(title.clientWidth);
    expect(description.scrollWidth).toBeLessThanOrEqual(
      description.clientWidth,
    );
    expect(description.getBoundingClientRect().width).toBeLessThanOrEqual(126);
  }, 20000);

  it("accepts the visible one-pixel font metric for an unconstrained line", async () => {
    const doc = await makeDocument(
      '<div style="padding-top:20px"><p id="metric" style="font:12px/16px IBM Plex Sans KR;margin:0">주식 코파일럿</p></div>',
    );
    await loadProbeFont(doc);
    const text = doc.querySelector<HTMLElement>("#metric");
    if (text === null) throw new Error("text missing");
    expect(text.scrollHeight - text.clientHeight).toBe(1);
    expect(inspectOverflow(doc)).not.toContainEqual({
      kind: "unauthorized-overflow",
      element: "p#metric",
    });
  });

  it("does not report a parent for its child's visible one-pixel font metric", async () => {
    const doc = await makeDocument(
      '<div id="metric-parent" style="padding-top:20px"><p id="metric-child" style="font:12px/16px IBM Plex Sans KR;margin:0">주식 코파일럿</p></div>',
    );
    await loadProbeFont(doc);
    const parent = doc.querySelector<HTMLElement>("#metric-parent");
    const child = doc.querySelector<HTMLElement>("#metric-child");
    if (parent === null || child === null) throw new Error("metric missing");
    expect(parent.scrollHeight - parent.clientHeight).toBe(1);
    expect(child.scrollHeight - child.clientHeight).toBe(1);
    expect(inspectOverflow(doc)).toEqual([]);
  });

  it("does not report a chain of parents for the same visible font metric", async () => {
    const doc = await makeDocument(
      '<div style="padding-top:20px"><section id="metric-ancestor"><div id="metric-wrapper"><p style="font:12px/16px IBM Plex Sans KR;margin:0">주식 코파일럿</p></div></section></div>',
    );
    await loadProbeFont(doc);
    const ancestor = doc.querySelector<HTMLElement>("#metric-ancestor");
    const wrapper = doc.querySelector<HTMLElement>("#metric-wrapper");
    if (ancestor === null || wrapper === null)
      throw new Error("metric missing");
    expect(ancestor.scrollHeight - ancestor.clientHeight).toBe(1);
    expect(wrapper.scrollHeight - wrapper.clientHeight).toBe(1);
    expect(inspectOverflow(doc)).toEqual([]);
  });

  it.each(["contain:paint", "clip-path:inset(0)"])(
    "still detects a font metric clipped by the text element's %s",
    async (clip) => {
      const doc = await makeDocument(
        `<div style="padding-top:20px"><p id="own-clip" style="font:12px/16px IBM Plex Sans KR;margin:0;${clip}">주식 코파일럿</p></div>`,
      );
      await loadProbeFont(doc);
      const text = doc.querySelector<HTMLElement>("#own-clip");
      if (text === null) throw new Error("text missing");
      expect(text.scrollHeight - text.clientHeight).toBe(1);
      expect(inspectOverflow(doc)).toContainEqual({
        kind: "unauthorized-overflow",
        element: "p#own-clip",
      });
    },
  );

  it.each(["contain:paint", "clip-path:inset(0)"])(
    "still detects a font metric clipped by its parent's %s",
    async (clip) => {
      const doc = await makeDocument(
        `<div style="padding-top:20px"><div id="parent-clip" style="height:16px;${clip}"><p style="font:12px/16px IBM Plex Sans KR;margin:0">주식 코파일럿</p></div></div>`,
      );
      await loadProbeFont(doc);
      const parent = doc.querySelector<HTMLElement>("#parent-clip");
      if (parent === null) throw new Error("parent missing");
      expect(parent.scrollHeight - parent.clientHeight).toBe(1);
      expect(inspectOverflow(doc)).toContainEqual({
        kind: "unauthorized-overflow",
        element: "div#parent-clip",
      });
    },
  );

  it("still detects a real one-pixel child overflow without clipping", async () => {
    const doc = await makeDocument(
      '<div id="real-visible" style="height:15px"><div style="height:16px"></div></div>',
    );
    const parent = doc.querySelector<HTMLElement>("#real-visible");
    if (parent === null) throw new Error("parent missing");
    expect(parent.scrollHeight - parent.clientHeight).toBe(1);
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "unauthorized-overflow",
      element: "div#real-visible",
    });
  });

  it.each(["::before", "::after"])(
    "still detects generated content %s overflowing beside a font metric",
    async (pseudo) => {
      const doc = await makeDocument(
        `<style>#generated-metric${pseudo}{content:"";position:absolute;top:16px;width:1px;height:1px}</style><div id="generated-metric" style="position:relative;top:20px;height:16px"><p style="font:12px/16px IBM Plex Sans KR;margin:0">주식 코파일럿</p></div>`,
      );
      await loadProbeFont(doc);
      const parent = doc.querySelector<HTMLElement>("#generated-metric");
      if (parent === null) throw new Error("parent missing");
      expect(parent.scrollHeight - parent.clientHeight).toBe(1);
      expect(inspectOverflow(doc)).toContainEqual({
        kind: "unauthorized-overflow",
        element: "div#generated-metric",
      });
    },
  );

  it("still detects a metric child positioned outside its parent", async () => {
    const doc = await makeDocument(
      '<div style="padding:20px"><div id="metric-edge" style="width:100px;height:16px"><p id="metric-outside" style="position:relative;left:-1px;font:12px/16px IBM Plex Sans KR;margin:0">주식 코파일럿</p></div></div>',
    );
    await loadProbeFont(doc);
    const parent = doc.querySelector<HTMLElement>("#metric-edge");
    const child = doc.querySelector<HTMLElement>("#metric-outside");
    if (parent === null || child === null) throw new Error("metric missing");
    expect(parent.scrollHeight - parent.clientHeight).toBe(1);
    expect(parent.scrollWidth).toBe(parent.clientWidth);
    expect(child.getBoundingClientRect().left).toBeLessThan(
      parent.getBoundingClientRect().left,
    );
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "unauthorized-overflow",
      element: "div#metric-edge",
    });
  });

  it("distinguishes overflowing elements of the same tag in the catalog", async () => {
    const first = await makeDocument(
      '<div><span style="display:inline-block;width:40px;white-space:nowrap">longlonglonglong</span><span style="display:inline-block;width:40px;white-space:nowrap">ok</span></div>',
    );
    const second = await makeDocument(
      '<div><span style="display:inline-block;width:40px;white-space:nowrap">ok</span><span style="display:inline-block;width:40px;white-space:nowrap">longlonglonglong</span></div>',
    );
    const a = inspectOverflow(first, identifyOverflowElement).filter(
      (finding) => finding.kind === "unauthorized-overflow",
    );
    const b = inspectOverflow(second, identifyOverflowElement).filter(
      (finding) => finding.kind === "unauthorized-overflow",
    );
    expect(a).toHaveLength(1);
    expect(b).toHaveLength(1);
    expect(a).not.toEqual(b);
  });

  it("accepts the same visible font metric across wrapped lines", async () => {
    const doc = await makeDocument(
      '<div style="padding-top:20px"><p id="wrapped-metric" style="width:70px;font:12px/16px IBM Plex Sans KR;margin:0">주식 코파일럿 주식 코파일럿 주식 코파일럿</p></div>',
    );
    await loadProbeFont(doc);
    const text = doc.querySelector<HTMLElement>("#wrapped-metric");
    if (text === null) throw new Error("text missing");
    const range = doc.createRange();
    range.selectNodeContents(text);
    expect(range.getClientRects().length).toBeGreaterThan(1);
    expect(text.scrollHeight - text.clientHeight).toBe(1);
    expect(inspectOverflow(doc)).not.toContainEqual({
      kind: "unauthorized-overflow",
      element: "p#wrapped-metric",
    });
  });

  it("accepts a visible font metric in a fixed dialog with body scroll locked", async () => {
    const doc = await makeDocument(
      '<div role="dialog" style="position:fixed;left:100px;top:100px;width:200px"><p id="dialog-metric" style="font:12px/16px IBM Plex Sans KR;margin:0">주식 코파일럿</p></div>',
    );
    await loadProbeFont(doc);
    doc.body.style.height = "20px";
    doc.body.style.overflow = "hidden";
    const text = doc.querySelector<HTMLElement>("#dialog-metric");
    if (text === null) throw new Error("text missing");
    expect(text.scrollHeight - text.clientHeight).toBe(1);
    expect(doc.elementFromPoint(105, 105)?.id).toBe("dialog-metric");
    expect(inspectOverflow(doc)).not.toContainEqual({
      kind: "unauthorized-overflow",
      element: "p#dialog-metric",
    });
  });

  it("still detects the font metric when body paint containment clips it", async () => {
    const doc = await makeDocument(
      '<div style="position:fixed;left:100px;top:100px;width:200px"><p id="paint-metric" style="font:12px/16px IBM Plex Sans KR;margin:0">주식 코파일럿</p></div>',
    );
    await loadProbeFont(doc);
    doc.body.style.height = "20px";
    doc.body.style.overflow = "hidden";
    doc.body.style.contain = "paint";
    const text = doc.querySelector<HTMLElement>("#paint-metric");
    if (text === null) throw new Error("text missing");
    expect(text.scrollHeight - text.clientHeight).toBe(1);
    expect(doc.elementFromPoint(105, 105)?.id).not.toBe("paint-metric");
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "unauthorized-overflow",
      element: "p#paint-metric",
    });
  });

  it("still detects horizontal overflow beside the font metric", async () => {
    const doc = await makeDocument(
      '<div style="padding-top:20px"><p id="wide-metric" style="width:30px;white-space:nowrap;font:12px/16px IBM Plex Sans KR;margin:0">주식 코파일럿</p></div>',
    );
    await loadProbeFont(doc);
    const text = doc.querySelector<HTMLElement>("#wide-metric");
    if (text === null) throw new Error("text missing");
    expect(text.scrollWidth).toBeGreaterThan(text.clientWidth);
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "unauthorized-overflow",
      element: "p#wide-metric",
    });
  });

  it("still detects one pixel of real child overflow", async () => {
    const doc = await makeDocument(
      '<div id="real-one-pixel" style="height:15px;overflow:hidden"><div style="height:16px"></div></div>',
    );
    const container = doc.querySelector<HTMLElement>("#real-one-pixel");
    if (container === null) throw new Error("container missing");
    expect(container.scrollHeight - container.clientHeight).toBe(1);
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "unauthorized-overflow",
      element: "div#real-one-pixel",
    });
  });

  it("still detects the font metric when an ancestor clips the text", async () => {
    const doc = await makeDocument(
      '<div style="padding-top:20px"><div style="height:16px;overflow:hidden"><p id="clipped-metric" style="font:12px/16px IBM Plex Sans KR;margin:0">주식 코파일럿</p></div></div>',
    );
    await loadProbeFont(doc);
    const text = doc.querySelector<HTMLElement>("#clipped-metric");
    if (text === null) throw new Error("text missing");
    expect(text.scrollHeight - text.clientHeight).toBe(1);
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "unauthorized-overflow",
      element: "p#clipped-metric",
    });
  });

  it("does not report the clipped content of a visually hidden accessible table", async () => {
    const doc = await makeDocument(
      '<div id="sr-wrapper" class="sr-only" style="position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0"><table id="sr-table" class="sr-only" style="position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0"><caption>접근성 표</caption><tbody><tr><td>읽을 수 있는 긴 값</td></tr></tbody></table></div>',
    );
    const findings = inspectOverflow(doc);
    expect(findings).not.toContainEqual({
      kind: "unauthorized-overflow",
      element: "div#sr-wrapper",
    });
    expect(findings).not.toContainEqual({
      kind: "clipped",
      element: "table#sr-table",
    });
  });

  it("detects a visible table that is falsely marked as screen-reader-only", async () => {
    const doc = await makeDocument(
      '<div id="fake-wrapper" class="sr-only" style="width:40px;height:30px;overflow:hidden"><table class="sr-only"><tbody><tr><td>longlonglonglonglonglong</td></tr></tbody></table></div>',
    );
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "unauthorized-overflow",
      element: "div#fake-wrapper",
    });
  });

  it("detects overflow when a marked table is not fully clipped by clip-path", async () => {
    const doc = await makeDocument(
      '<div id="partly-hidden" class="sr-only" style="position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(0%)"><table class="sr-only" style="position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(0%)"><tbody><tr><td>longlonglonglonglonglong</td></tr></tbody></table></div>',
    );
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "unauthorized-overflow",
      element: "div#partly-hidden",
    });
  });

  it("detects overflowing content outside a visually hidden accessible table", async () => {
    const doc = await makeDocument(
      '<div class="sr-only" style="position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0"><table class="sr-only" style="position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0"><tbody><tr><td>표</td></tr></tbody></table></div><div id="real-overflow" style="width:40px;white-space:nowrap">longlonglonglonglonglong</div>',
    );
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "unauthorized-overflow",
      element: "div#real-overflow",
    });
  });

  it("does not treat a hidden non-table element as an accessible table", async () => {
    const doc = await makeDocument(
      '<div id="hidden-non-table" class="sr-only" style="position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0">longlonglonglonglonglong</div>',
    );
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "unauthorized-overflow",
      element: "div#hidden-non-table",
    });
  });

  it.each(["text", "search", "tel", "url", "email", "password"])(
    "accepts the editable %s input's own horizontal scrolling",
    async (type) => {
      const doc = await makeDocument(
        `<input id="editor" type="${type}" value="longlonglonglonglonglonglonglong" style="width:60px">`,
      );
      const input = doc.querySelector<HTMLInputElement>("#editor");
      if (input === null) throw new Error("input missing");
      expect(input.scrollWidth).toBeGreaterThan(input.clientWidth);
      expect(inspectOverflow(doc)).not.toContainEqual({
        kind: "unauthorized-overflow",
        element: "input#editor",
      });
    },
  );

  it.each(["auto", "scroll"])(
    "accepts an editable textarea's own vertical scrolling with %s",
    async (overflow) => {
      const doc = await makeDocument(
        `<textarea id="editor" style="width:120px;height:40px;overflow-y:${overflow}">one\ntwo\nthree\nfour\nfive\nsix</textarea>`,
      );
      const input = doc.querySelector<HTMLTextAreaElement>("#editor");
      if (input === null) throw new Error("textarea missing");
      expect(input.scrollHeight).toBeGreaterThan(input.clientHeight);
      expect(inspectOverflow(doc)).not.toContainEqual({
        kind: "unauthorized-overflow",
        element: "textarea#editor",
      });
    },
  );

  it("still detects an input box clipped by its parent", async () => {
    const doc = await makeDocument(
      '<div style="width:40px;overflow:hidden"><input id="wide-input" type="text" value="longlonglonglonglonglong" style="width:120px"></div>',
    );
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "clipped",
      element: "input#wide-input",
    });
  });

  it("still detects a textarea's horizontal content overflow", async () => {
    const doc = await makeDocument(
      '<textarea id="wide-textarea" wrap="off" style="width:60px;height:40px;overflow:auto">longlonglonglonglonglonglonglong</textarea>',
    );
    const input = doc.querySelector<HTMLTextAreaElement>("#wide-textarea");
    if (input === null) throw new Error("textarea missing");
    expect(input.scrollWidth).toBeGreaterThan(input.clientWidth);
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "unauthorized-overflow",
      element: "textarea#wide-textarea",
    });
  });

  it("still detects horizontal textarea overflow even with a viewport marker", async () => {
    const doc = await makeDocument(
      '<textarea id="marked-textarea" data-scroll-viewport wrap="off" style="width:60px;height:40px;overflow:auto">longlonglonglonglonglonglonglong</textarea>',
    );
    const input = doc.querySelector<HTMLTextAreaElement>("#marked-textarea");
    if (input === null) throw new Error("textarea missing");
    expect(input.scrollWidth).toBeGreaterThan(input.clientWidth);
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "unauthorized-overflow",
      element: "textarea#marked-textarea",
    });
  });

  it.each(["number", "unsupported"])(
    "accepts editable %s input content scrolling",
    async (type) => {
      const doc = await makeDocument(
        `<input id="numeric-editor" type="${type}" value="123456789012345678901234567890" style="width:50px">`,
      );
      const input = doc.querySelector<HTMLInputElement>("#numeric-editor");
      if (input === null) throw new Error("input missing");
      expect(input.scrollWidth).toBeGreaterThan(input.clientWidth);
      expect(inspectOverflow(doc)).not.toContainEqual({
        kind: "unauthorized-overflow",
        element: "input#numeric-editor",
      });
    },
  );

  it.each([
    [
      "vertical",
      '<textarea id="truncated-editor" data-truncate title="one two three four five" style="width:120px;height:30px;overflow:hidden;text-overflow:ellipsis">one\ntwo\nthree\nfour\nfive</textarea>',
    ],
    [
      "horizontal",
      '<textarea id="truncated-editor" data-truncate wrap="off" title="longlonglonglonglonglonglonglong" style="width:60px;height:50px;overflow:hidden;text-overflow:ellipsis">longlonglonglonglonglonglonglong</textarea>',
    ],
  ])(
    "still detects %s textarea overflow with a truncation marker",
    async (axis, markup) => {
      const doc = await makeDocument(markup);
      const input = doc.querySelector<HTMLTextAreaElement>("#truncated-editor");
      if (input === null) throw new Error("textarea missing");
      expect(getComputedStyle(input).overflowY).toBe("hidden");
      expect(
        axis === "vertical"
          ? input.scrollHeight > input.clientHeight
          : input.scrollWidth > input.clientWidth,
      ).toBe(true);
      expect(inspectOverflow(doc)).toContainEqual({
        kind: "unauthorized-overflow",
        element: "textarea#truncated-editor",
      });
    },
  );

  it("still detects textarea vertical overflow with overflow-y hidden", async () => {
    const doc = await makeDocument(
      '<textarea id="clipped-textarea" style="width:120px;height:40px;overflow:hidden">one\ntwo\nthree\nfour\nfive\nsix</textarea>',
    );
    const input = doc.querySelector<HTMLTextAreaElement>("#clipped-textarea");
    if (input === null) throw new Error("textarea missing");
    expect(getComputedStyle(input).overflowY).toBe("hidden");
    expect(input.scrollHeight).toBeGreaterThan(input.clientHeight);
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "unauthorized-overflow",
      element: "textarea#clipped-textarea",
    });
  });

  it("still detects textarea vertical overflow when computed overflow-y is visible", async () => {
    const doc = await makeDocument(
      '<textarea id="visible-textarea" style="width:120px;height:40px;overflow:hidden">one\ntwo\nthree\nfour\nfive\nsix</textarea>',
    );
    const input = doc.querySelector<HTMLTextAreaElement>("#visible-textarea");
    if (input === null) throw new Error("textarea missing");
    const original = globalThis.getComputedStyle;
    const styles = vi
      .spyOn(globalThis, "getComputedStyle")
      .mockImplementation((element, pseudo) => {
        const style = original(element, pseudo);
        if (element !== input) return style;
        return new Proxy(style, {
          get(target, property) {
            if (property === "overflowY") return "visible";
            const value = Reflect.get(target, property, target);
            return typeof value === "function" ? value.bind(target) : value;
          },
        });
      });
    try {
      expect(input.scrollHeight).toBeGreaterThan(input.clientHeight);
      expect(inspectOverflow(doc)).toContainEqual({
        kind: "unauthorized-overflow",
        element: "textarea#visible-textarea",
      });
    } finally {
      styles.mockRestore();
    }
  });

  it("still detects the same multiline content in an ordinary element", async () => {
    const doc = await makeDocument(
      '<div id="ordinary" style="width:120px;height:40px;white-space:pre-wrap">one\ntwo\nthree\nfour\nfive\nsix</div>',
    );
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "unauthorized-overflow",
      element: "div#ordinary",
    });
  });

  it.each(["label", "hint", "error"])(
    "still detects overflowing input %s text",
    async (part) => {
      const doc = await makeDocument(
        `<label for="editor" id="label" style="display:block;width:40px;white-space:nowrap">${part === "label" ? "longlonglonglonglong" : "label"}</label><input id="editor" style="width:60px"><div id="hint" style="width:40px;white-space:nowrap">${part === "hint" ? "longlonglonglonglong" : "hint"}</div><div id="error" style="width:40px;white-space:nowrap">${part === "error" ? "longlonglonglonglong" : "error"}</div>`,
      );
      expect(inspectOverflow(doc)).toContainEqual({
        kind: "unauthorized-overflow",
        element: `${part === "label" ? "label" : "div"}#${part}`,
      });
    },
  );

  it("still detects a readonly input's overflowing content", async () => {
    const doc = await makeDocument(
      '<input id="readonly-input" readonly value="longlonglonglonglonglonglonglong" style="width:60px">',
    );
    const input = doc.querySelector<HTMLInputElement>("#readonly-input");
    if (input === null) throw new Error("input missing");
    expect(input.scrollWidth).toBeGreaterThan(input.clientWidth);
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "unauthorized-overflow",
      element: "input#readonly-input",
    });
  });

  it("still detects a disabled input's overflowing content", async () => {
    const doc = await makeDocument(
      '<input id="disabled-input" disabled value="longlonglonglonglonglonglonglong" style="width:60px">',
    );
    const input = doc.querySelector<HTMLInputElement>("#disabled-input");
    if (input === null) throw new Error("input missing");
    expect(input.scrollWidth).toBeGreaterThan(input.clientWidth);
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "unauthorized-overflow",
      element: "input#disabled-input",
    });
  });

  it.each([
    [
      "input itself",
      '<input id="inert-field" inert value="longlonglonglonglonglonglong" style="width:60px">',
      "input#inert-field",
    ],
    [
      "input ancestor",
      '<div inert><input id="inert-field" value="longlonglonglonglonglonglong" style="width:60px"></div>',
      "input#inert-field",
    ],
    [
      "textarea itself",
      '<textarea id="inert-field" inert style="width:120px;height:35px;overflow:auto">one\ntwo\nthree\nfour\nfive</textarea>',
      "textarea#inert-field",
    ],
    [
      "textarea ancestor",
      '<div inert><textarea id="inert-field" style="width:120px;height:35px;overflow:auto">one\ntwo\nthree\nfour\nfive</textarea></div>',
      "textarea#inert-field",
    ],
  ])(
    "still detects %s content overflow when inert",
    async (_label, markup, element) => {
      const doc = await makeDocument(markup);
      const field = doc.querySelector<HTMLElement>("#inert-field");
      if (field === null) throw new Error("field missing");
      expect(
        field.scrollWidth > field.clientWidth ||
          field.scrollHeight > field.clientHeight,
      ).toBe(true);
      field.focus();
      expect(doc.activeElement).not.toBe(field);
      expect(inspectOverflow(doc)).toContainEqual({
        kind: "unauthorized-overflow",
        element,
      });
    },
  );

  it.each([
    {
      name: "document horizontal scroll",
      expected: "document-scroll",
      html: 'data-scroll-viewport=""',
      body: '<div style="width:800px;height:10px"></div>',
    },
    {
      name: "long text in a fixed height",
      expected: "unauthorized-overflow",
      body: '<div style="width:120px;height:20px">긴 글 긴 글 긴 글 긴 글 긴 글 긴 글 긴 글 긴 글</div>',
    },
    {
      name: "flex child without min-width zero",
      expected: "unauthorized-overflow",
      body: '<div style="display:flex;width:120px"><div style="white-space:nowrap">verylongunbreakableenglishwordverylongunbreakableenglishword</div></div>',
    },
    {
      name: "popover clipped by ancestor",
      expected: "clipped",
      body: '<div style="position:absolute;left:50px;top:50px;overflow:hidden;width:100px;height:80px"><div role="dialog" style="position:absolute;left:-20px;top:10px;width:40px;height:30px">팝업</div></div>',
    },
    {
      name: "truncation without full text tooltip",
      expected: "missing-tooltip",
      body: '<div data-truncate style="width:60px;overflow:hidden;white-space:nowrap;text-overflow:ellipsis">longlonglonglonglonglong</div>',
    },
    {
      name: "nested vertical scroll viewports",
      expected: "nested-scroll",
      body: '<div data-scroll-viewport style="width:180px;height:120px;overflow:auto"><div data-scroll-viewport style="width:150px;height:80px;overflow:auto"><div style="height:160px"></div></div><div style="height:100px"></div></div>',
    },
    {
      name: "portal outside the window",
      expected: "outside-window",
      body: '<div role="dialog" style="position:fixed;left:620px;top:40px;width:60px;height:40px">팝업</div>',
    },
    {
      name: "overlapping sibling controls",
      expected: "overlapping-controls",
      body: '<div><button style="width:100px;height:30px">하나</button><button style="width:100px;height:30px;margin-left:-20px">둘</button></div>',
    },
  ])("isolates $name", async ({ body, expected, ...rest }) => {
    const doc = await makeDocument(body, "html" in rest ? rest.html : "");
    if (expected === "document-scroll") {
      doc.documentElement.style.overflow = "auto";
      const content = doc.body.firstElementChild;
      if (content !== null) doc.documentElement.append(content);
    }
    const findings = inspectOverflow(doc);
    expect(findings.map((finding) => finding.kind)).toContain(expected);
    expect(findings.filter((finding) => finding.kind !== expected)).toEqual([]);
  });

  it("accepts contained controls and reachable viewport content", async () => {
    const doc = await makeDocument(
      '<div data-scroll-viewport style="overflow:auto;width:160px;height:80px"><div style="height:160px">내용</div></div><div><button>하나</button><button>둘</button></div>',
    );
    expect(inspectOverflow(doc)).toEqual([]);
  });

  it("accepts a tooltip containing the complete truncated text", async () => {
    const doc = await makeDocument(
      '<div data-truncate title="longlonglonglong" style="width:40px;overflow:hidden;white-space:nowrap;text-overflow:ellipsis">longlonglonglong</div>',
    );
    expect(inspectOverflow(doc)).toEqual([]);
  });

  it("detects vertical nesting through an intermediate horizontal viewport", async () => {
    const doc = await makeDocument(
      '<div data-scroll-viewport style="overflow-y:auto;width:180px;height:120px"><div data-scroll-viewport style="overflow-x:auto;overflow-y:hidden;width:170px;height:100px"><div data-scroll-viewport style="overflow-y:auto;width:150px;height:80px"><div style="height:160px"></div></div></div><div style="height:100px"></div></div>',
    );
    expect(inspectOverflow(doc).map((finding) => finding.kind)).toContain(
      "nested-scroll",
    );
  });

  it("permits the ChatInput textarea as the documented nested-scroll exception", async () => {
    const doc = await makeDocument(
      '<div data-scroll-viewport style="overflow:auto;width:180px;height:100px"><textarea class="chat-input" data-scroll-viewport style="overflow:auto;width:150px;height:80px">내용</textarea><div style="height:100px"></div></div>',
    );
    expect(inspectOverflow(doc).map((finding) => finding.kind)).not.toContain(
      "nested-scroll",
    );
  });

  it("still rejects another nested textarea viewport", async () => {
    const doc = await makeDocument(
      '<div data-scroll-viewport style="overflow:auto;width:180px;height:100px"><textarea data-scroll-viewport style="overflow:auto;width:150px;height:80px">내용</textarea><div style="height:100px"></div></div>',
    );
    expect(inspectOverflow(doc).map((finding) => finding.kind)).toContain(
      "nested-scroll",
    );
  });

  it("detects document vertical scroll independently", async () => {
    const doc = await makeDocument(
      '<div style="height:800px;width:10px"></div>',
      'data-scroll-viewport=""',
    );
    doc.documentElement.style.overflow = "auto";
    const content = doc.body.firstElementChild;
    if (content !== null) doc.documentElement.append(content);
    expect([
      ...new Set(inspectOverflow(doc).map((finding) => finding.kind)),
    ]).toEqual(["document-scroll"]);
  });

  it("rejects a tooltip that omits part of the truncated text", async () => {
    const doc = await makeDocument(
      '<div data-truncate title="longlong" style="width:40px;overflow:hidden;white-space:nowrap">longlonglonglong</div>',
    );
    expect(inspectOverflow(doc).map((finding) => finding.kind)).toContain(
      "missing-tooltip",
    );
  });

  it.each([
    ["left", "left:-20px;top:10px"],
    ["right", "left:80px;top:10px"],
    ["top", "left:10px;top:-20px"],
    ["bottom", "left:10px;top:70px"],
  ])("detects clipping at the %s edge", async (_name, position) => {
    const doc = await makeDocument(
      `<div data-truncate title="팝업" style="position:absolute;left:100px;top:100px;overflow:hidden;width:100px;height:80px"><div style="position:absolute;${position};width:40px;height:30px">팝업</div></div>`,
    );
    expect(inspectOverflow(doc).map((finding) => finding.kind)).toContain(
      "clipped",
    );
  });

  it.each([
    ["left", "left:-1px;top:100px"],
    ["right", "left:620px;top:100px"],
    ["top", "left:100px;top:-1px"],
    ["bottom", "left:100px;top:460px"],
  ])("detects an overlay beyond the %s edge", async (_name, position) => {
    const doc = await makeDocument(
      `<div role="dialog" style="position:fixed;${position};width:40px;height:30px">팝업</div>`,
    );
    expect(inspectOverflow(doc).map((finding) => finding.kind)).toContain(
      "outside-window",
    );
  });

  it.each([
    ["left", "left:20px;top:40px"],
    ["right", "left:100px;top:40px"],
    ["top", "left:40px;top:20px"],
    ["bottom", "left:40px;top:60px"],
  ])("detects overlapping controls on the %s side", async (_name, position) => {
    const doc = await makeDocument(
      `<div style="position:relative;width:240px;height:160px"><button style="position:absolute;left:40px;top:40px;width:100px;height:40px">하나</button><button style="position:absolute;${position};width:60px;height:30px">둘</button></div>`,
    );
    expect(inspectOverflow(doc).map((finding) => finding.kind)).toContain(
      "overlapping-controls",
    );
  });

  it("allows adjacent controls that only touch at an edge", async () => {
    const doc = await makeDocument(
      '<div style="position:relative;width:240px;height:80px"><button style="position:absolute;left:0;top:0;width:100px;height:40px">하나</button><button style="position:absolute;left:100px;top:0;width:60px;height:40px">둘</button></div>',
    );
    expect(inspectOverflow(doc).map((finding) => finding.kind)).not.toContain(
      "overlapping-controls",
    );
  });
});

describe("review regressions", () => {
  it("measures the fixtures in standards mode", async () => {
    const doc = await makeDocument("<div>내용</div>");
    expect(doc.compatMode).toBe("CSS1Compat");
  });

  it("detects clipping on the non-scrollable axis of a viewport", async () => {
    const doc = await makeDocument(
      '<div data-scroll-viewport style="overflow-x:hidden;overflow-y:auto;width:100px;height:100px"><div id="wide-child" style="width:300px;height:20px"></div><div style="height:200px"></div></div>',
    );
    const parent = doc.querySelector("[data-scroll-viewport]");
    expect(parent).not.toBeNull();
    if (parent === null) throw new Error("viewport missing");
    expect(doc.defaultView?.getComputedStyle(parent).overflowX).toBe("hidden");
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "clipped",
      element: "div#wide-child",
    });
  });

  it("detects vertical clipping in a horizontally scrollable viewport", async () => {
    const doc = await makeDocument(
      '<div data-scroll-viewport style="overflow-x:auto;overflow-y:hidden;width:100px;height:100px"><div id="tall-child" style="width:200px;height:300px"></div></div>',
    );
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "clipped",
      element: "div#tall-child",
    });
  });

  it("rejects a child that extends before the scroll origin", async () => {
    const doc = await makeDocument(
      '<div data-scroll-viewport style="position:relative;overflow:auto;width:100px;height:100px"><div id="negative-child" style="position:absolute;left:-30px;top:10px;width:40px;height:20px"></div></div>',
    );
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "clipped",
      element: "div#negative-child",
    });
  });

  it("rejects a child above the vertical scroll origin", async () => {
    const doc = await makeDocument(
      '<div data-scroll-viewport style="position:relative;overflow:auto;width:100px;height:100px"><div id="negative-top" style="position:absolute;left:10px;top:-30px;width:40px;height:40px"></div></div>',
    );
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "clipped",
      element: "div#negative-top",
    });
  });

  it("accepts horizontal content that can be reached by scrolling", async () => {
    const doc = await makeDocument(
      '<div id="viewport" data-scroll-viewport style="overflow:auto;width:100px;height:100px"><div id="wide-child" style="width:300px;height:20px"></div></div>',
    );
    const parent = doc.querySelector<HTMLElement>("#viewport");
    const child = doc.querySelector<HTMLElement>("#wide-child");
    if (parent === null || child === null) throw new Error("viewport missing");
    expect(inspectOverflow(doc)).not.toContainEqual({
      kind: "clipped",
      element: "div#wide-child",
    });
    parent.scrollLeft = 200;
    expect(parent.scrollLeft).toBeGreaterThan(0);
    expect(child.getBoundingClientRect().right).toBeLessThanOrEqual(
      parent.getBoundingClientRect().right,
    );
    expect(inspectOverflow(doc)).not.toContainEqual({
      kind: "clipped",
      element: "div#wide-child",
    });
  });

  it("accepts reachable right-to-left scroll content", async () => {
    const doc = await makeDocument(
      '<div id="viewport" dir="rtl" data-scroll-viewport style="overflow:auto;width:100px;height:100px"><div id="wide-child" style="width:300px;height:20px"></div></div>',
    );
    const parent = doc.querySelector<HTMLElement>("#viewport");
    const child = doc.querySelector<HTMLElement>("#wide-child");
    if (parent === null || child === null) throw new Error("viewport missing");
    expect(inspectOverflow(doc)).not.toContainEqual({
      kind: "clipped",
      element: "div#wide-child",
    });
    parent.scrollLeft = -200;
    expect(parent.scrollLeft).toBeLessThan(0);
    expect(child.getBoundingClientRect().left).toBeGreaterThanOrEqual(
      parent.getBoundingClientRect().left,
    );
    expect(inspectOverflow(doc)).not.toContainEqual({
      kind: "clipped",
      element: "div#wide-child",
    });
  });

  it("accepts reachable content in a reversed horizontal flex viewport", async () => {
    const doc = await makeDocument(
      '<div id="viewport" data-scroll-viewport style="display:flex;flex-direction:row-reverse;overflow:auto;width:100px;height:100px"><div id="wide-child" style="flex:none;width:300px;height:20px"></div></div>',
    );
    const parent = doc.querySelector<HTMLElement>("#viewport");
    if (parent === null) throw new Error("viewport missing");
    expect(inspectOverflow(doc)).not.toContainEqual({
      kind: "clipped",
      element: "div#wide-child",
    });
    parent.scrollLeft = -200;
    expect(parent.scrollLeft).toBeLessThan(0);
    expect(inspectOverflow(doc)).not.toContainEqual({
      kind: "clipped",
      element: "div#wide-child",
    });
  });

  it("accepts reachable content in a reversed vertical flex viewport", async () => {
    const doc = await makeDocument(
      '<div id="viewport" data-scroll-viewport style="display:flex;flex-direction:column-reverse;overflow:auto;width:100px;height:100px"><div id="tall-child" style="flex:none;width:20px;height:300px"></div></div>',
    );
    const parent = doc.querySelector<HTMLElement>("#viewport");
    if (parent === null) throw new Error("viewport missing");
    expect(inspectOverflow(doc)).not.toContainEqual({
      kind: "clipped",
      element: "div#tall-child",
    });
    parent.scrollTop = -200;
    expect(parent.scrollTop).toBeLessThan(0);
    expect(inspectOverflow(doc)).not.toContainEqual({
      kind: "clipped",
      element: "div#tall-child",
    });
  });

  it("accepts reachable reversed flex content in right-to-left layout", async () => {
    const doc = await makeDocument(
      '<div id="viewport" dir="rtl" data-scroll-viewport style="display:flex;flex-direction:row-reverse;overflow:auto;width:100px;height:100px"><div id="wide-child" style="flex:none;width:300px;height:20px"></div></div>',
    );
    const parent = doc.querySelector<HTMLElement>("#viewport");
    if (parent === null) throw new Error("viewport missing");
    expect(inspectOverflow(doc)).not.toContainEqual({
      kind: "clipped",
      element: "div#wide-child",
    });
    parent.scrollLeft = 200;
    expect(parent.scrollLeft).toBeGreaterThan(0);
    expect(inspectOverflow(doc)).not.toContainEqual({
      kind: "clipped",
      element: "div#wide-child",
    });
  });

  it("detects nested native textarea scrolling except ChatInput", async () => {
    const doc = await makeDocument(
      '<div data-scroll-viewport style="overflow:auto;width:180px;height:100px"><textarea id="ordinary" style="overflow:auto;width:150px;height:30px"></textarea><textarea id="chat" class="chat-input" style="overflow:auto;width:150px;height:30px"></textarea><div style="height:100px"></div></div>',
    );
    const findings = inspectOverflow(doc);
    expect(findings).toContainEqual({
      kind: "nested-scroll",
      element: "textarea#ordinary",
    });
    expect(findings).not.toContainEqual({
      kind: "nested-scroll",
      element: "textarea#chat",
    });
  });

  it("reports a document without a layout viewport", () => {
    const doc = document.implementation.createHTMLDocument("inert");
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "layout-unavailable",
      element: "html",
    });
  });

  it("reports a hidden document root as unavailable layout", async () => {
    const doc = await makeDocument("<div>내용</div>");
    doc.documentElement.style.display = "none";
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "layout-unavailable",
      element: "html",
    });
  });

  it("reports a hidden document body as unavailable layout", async () => {
    const doc = await makeDocument("<div>내용</div>");
    doc.body.style.display = "none";
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "layout-unavailable",
      element: "body",
    });
  });

  it("does not clip a visible fixed dialog at a short scroll-locked body", async () => {
    const doc = await makeDocument(
      '<div role="dialog" id="modal" style="position:fixed;left:100px;top:100px;width:100px;height:100px"><button id="inside" style="width:40px;height:30px">확인</button></div>',
    );
    doc.body.style.height = "20px";
    doc.body.style.overflow = "hidden";
    expect(doc.elementFromPoint(110, 110)?.id).toBe("inside");
    expect(
      inspectOverflow(doc).filter((finding) => finding.kind === "clipped"),
    ).toEqual([]);
  });

  it("does not clip a fixed element at an unrelated overflow ancestor", async () => {
    const doc = await makeDocument(
      '<div style="position:relative;overflow:hidden;width:60px;height:60px"><button id="fixed-child" style="position:fixed;left:100px;top:100px;width:60px;height:30px">확인</button></div>',
    );
    expect(doc.elementFromPoint(110, 110)?.id).toBe("fixed-child");
    expect(
      inspectOverflow(doc).filter((finding) => finding.kind === "clipped"),
    ).toEqual([]);
  });

  it("detects a fixed element clipped by its transformed containing block", async () => {
    const doc = await makeDocument(
      '<div style="position:relative;transform:translateZ(0);overflow:hidden;width:60px;height:60px"><button id="fixed-child" style="position:fixed;left:50px;top:10px;width:40px;height:30px">확인</button></div>',
    );
    expect(doc.elementFromPoint(70, 20)?.id).not.toBe("fixed-child");
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "clipped",
      element: "button#fixed-child",
    });
  });

  it.each([
    ["translate", "translate:0px", "fixed"],
    ["translate", "translate:0px", "absolute"],
    ["rotate", "rotate:0deg", "fixed"],
    ["rotate", "rotate:0deg", "absolute"],
    ["scale", "scale:1", "fixed"],
    ["scale", "scale:1", "absolute"],
    ["will-change translate", "will-change:translate", "fixed"],
    ["will-change rotate", "will-change:rotate", "fixed"],
    ["will-change scale", "will-change:scale", "fixed"],
  ])(
    "detects clipping through individual %s for %s as %s",
    async (_name, individualTransform, position) => {
      const doc = await makeDocument(
        `<div style="overflow:hidden;width:60px;height:60px"><div style="${individualTransform};width:60px;height:60px"><div id="positioned-child" style="position:${position};left:50px;top:10px;width:40px;height:30px;background:red"></div></div></div>`,
      );
      expect(doc.elementFromPoint(55, 15)?.id).toBe("positioned-child");
      expect(doc.elementFromPoint(70, 15)?.id).not.toBe("positioned-child");
      expect(inspectOverflow(doc)).toContainEqual({
        kind: "clipped",
        element: "div#positioned-child",
      });
    },
  );

  it.each([
    "perspective:1px",
    "filter:blur(0px)",
    "backdrop-filter:blur(0px)",
    "will-change:transform",
    "will-change:perspective",
    "will-change:filter",
    "will-change:backdrop-filter",
  ])("detects fixed-child clipping through %s", async (containingStyle) => {
    const doc = await makeDocument(
      `<div style="overflow:hidden;width:60px;height:60px"><div style="${containingStyle};width:60px;height:60px"><div id="fixed-effect" style="position:fixed;left:50px;top:10px;width:40px;height:30px;background:red"></div></div></div>`,
    );
    expect(doc.elementFromPoint(55, 15)?.id).toBe("fixed-effect");
    expect(doc.elementFromPoint(70, 15)?.id).not.toBe("fixed-effect");
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "clipped",
      element: "div#fixed-effect",
    });
  });

  it.each(["paint", "content", "strict"])(
    "detects clipping caused by %s containment",
    async (containment) => {
      const doc = await makeDocument(
        `<div style="contain:${containment};width:60px;height:60px"><div id="paint-child" style="position:absolute;left:50px;top:10px;width:40px;height:30px;background:red"></div></div>`,
      );
      expect(doc.elementFromPoint(55, 15)?.id).toBe("paint-child");
      expect(doc.elementFromPoint(70, 15)?.id).not.toBe("paint-child");
      expect(inspectOverflow(doc)).toContainEqual({
        kind: "clipped",
        element: "div#paint-child",
      });
    },
  );

  it("does not propagate body paint containment to the viewport", async () => {
    const doc = await makeDocument(
      '<div id="paint-child" style="position:fixed;left:50px;top:50px;width:30px;height:30px;background:red"></div>',
    );
    doc.body.style.height = "60px";
    doc.body.style.overflow = "hidden";
    doc.body.style.contain = "paint";
    expect(doc.elementFromPoint(55, 55)?.id).toBe("paint-child");
    expect(doc.elementFromPoint(55, 70)?.id).not.toBe("paint-child");
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "clipped",
      element: "div#paint-child",
    });
  });

  it("matches clipping behavior for a size container", async () => {
    const doc = await makeDocument(
      '<div id="container" style="position:relative;margin-left:100px;margin-top:100px;container-type:inline-size;overflow:hidden;width:60px;height:60px"><div id="fixed-child" style="position:fixed;left:50px;top:10px;width:40px;height:30px"></div></div>',
    );
    const container = doc.querySelector<HTMLElement>("#container");
    const child = doc.querySelector<HTMLElement>("#fixed-child");
    if (container === null || child === null)
      throw new Error("container missing");
    const origin = container.getBoundingClientRect();
    const bounds = child.getBoundingClientRect();
    const findings = inspectOverflow(doc);
    if (bounds.left === 50) {
      expect(doc.elementFromPoint(55, 15)?.id).toBe("fixed-child");
      expect(findings).not.toContainEqual({
        kind: "clipped",
        element: "div#fixed-child",
      });
    } else {
      expect(bounds.left).toBe(origin.left + 50);
      expect(
        doc.elementFromPoint(origin.left + 70, origin.top + 15)?.id,
      ).not.toBe("fixed-child");
      expect(findings).toContainEqual({
        kind: "clipped",
        element: "div#fixed-child",
      });
    }
  });

  it("detects clipping at the padding edge inside a border", async () => {
    const doc = await makeDocument(
      '<div style="position:absolute;left:100px;top:100px;width:100px;height:80px;border:10px solid;overflow:hidden"><div id="border-child" style="position:absolute;left:-5px;top:10px;width:20px;height:20px"></div></div>',
    );
    expect(doc.elementFromPoint(107, 125)?.id).not.toBe("border-child");
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "clipped",
      element: "div#border-child",
    });
  });

  it("distinguishes a flex minimum width from content that can wrap", async () => {
    const doc = await makeDocument(
      '<div style="display:flex;width:120px"><div id="flex-child" style="overflow-wrap:break-word">verylongunbreakableenglishwordverylongunbreakableenglishword</div></div>',
    );
    const child = doc.querySelector<HTMLElement>("#flex-child");
    expect(child).not.toBeNull();
    if (child === null) throw new Error("flex child missing");
    expect(child.getBoundingClientRect().width).toBeGreaterThan(120);
    expect(inspectOverflow(doc).map((finding) => finding.kind)).toContain(
      "unauthorized-overflow",
    );
    child.style.minWidth = "0";
    expect(child.getBoundingClientRect().width).toBeLessThanOrEqual(120);
    expect(inspectOverflow(doc)).toEqual([]);
  });

  it("detects an overlapping switch and button under the same parent", async () => {
    const doc = await makeDocument(
      '<div style="position:relative;width:160px;height:60px"><span role="switch" style="position:absolute;left:0;top:0;width:80px;height:30px"></span><button style="position:absolute;left:60px;top:0;width:80px;height:30px">확인</button></div>',
    );
    expect(inspectOverflow(doc).map((finding) => finding.kind)).toContain(
      "overlapping-controls",
    );
  });

  it("rejects a viewport marker on an element that cannot scroll", async () => {
    const doc = await makeDocument(
      '<div id="fake-viewport" data-scroll-viewport style="width:40px;white-space:nowrap">longlonglonglonglonglong</div>',
    );
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "unauthorized-overflow",
      element: "div#fake-viewport",
    });
  });

  it("rejects a viewport marker on a non-scrolling document body", async () => {
    const doc = await makeDocument(
      '<div style="width:800px;height:10px"></div>',
    );
    doc.body.setAttribute("data-scroll-viewport", "");
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "unauthorized-overflow",
      element: "body",
    });
  });

  it("rejects a truncation marker without truncating styles", async () => {
    const doc = await makeDocument(
      '<div id="fake-truncate" data-truncate title="longlonglonglong" style="width:40px;white-space:nowrap">longlonglonglong</div>',
    );
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "unauthorized-overflow",
      element: "div#fake-truncate",
    });
  });

  it("accepts the complete text from an associated tooltip", async () => {
    const doc = await makeDocument(
      '<div data-truncate aria-describedby="full" style="width:40px;overflow:hidden;white-space:nowrap;text-overflow:ellipsis">longlonglonglong</div><span id="full" role="tooltip" style="display:none">longlonglonglong</span>',
    );
    expect(inspectOverflow(doc)).toEqual([]);
  });

  it("rejects an associated tooltip that lacks the complete text", async () => {
    const doc = await makeDocument(
      '<div data-truncate aria-describedby="partial" style="width:40px;overflow:hidden;white-space:nowrap;text-overflow:ellipsis">longlonglonglong</div><span id="partial" role="tooltip" style="display:none">longlong</span>',
    );
    expect(inspectOverflow(doc).map((finding) => finding.kind)).toContain(
      "missing-tooltip",
    );
  });

  it("does not compare a control with a hidden sibling", async () => {
    const doc = await makeDocument(
      '<div style="position:relative;width:160px;height:60px"><button style="position:absolute;left:0;top:0;width:80px;height:30px">확인</button><button style="display:none;position:absolute;left:0;top:0;width:80px;height:30px">취소</button></div>',
    );
    expect(inspectOverflow(doc).map((finding) => finding.kind)).not.toContain(
      "overlapping-controls",
    );
  });

  it.each(["alertdialog", "menu", "listbox", "tooltip"])(
    "detects a %s outside the window",
    async (role) => {
      const doc = await makeDocument(
        `<div role="${role}" style="position:fixed;left:620px;top:40px;width:60px;height:40px">팝업</div>`,
      );
      expect(inspectOverflow(doc).map((finding) => finding.kind)).toContain(
        "outside-window",
      );
    },
  );

  it("detects a marked toast outside the window", async () => {
    const doc = await makeDocument(
      '<div role="status" data-overflow-overlay style="position:fixed;left:620px;top:40px;width:60px;height:40px">알림</div>',
    );
    expect(inspectOverflow(doc).map((finding) => finding.kind)).toContain(
      "outside-window",
    );
  });

  it.each([
    '<input style="position:absolute;left:60px;top:0;width:80px;height:30px">',
    '<select style="position:absolute;left:60px;top:0;width:80px;height:30px"><option>선택</option></select>',
    '<textarea style="position:absolute;left:60px;top:0;width:80px;height:30px"></textarea>',
    '<a href="#" style="position:absolute;left:60px;top:0;width:80px;height:30px">링크</a>',
    '<span role="button" style="position:absolute;left:60px;top:0;width:80px;height:30px">확인</span>',
  ])("detects overlapping control types", async (control) => {
    const doc = await makeDocument(
      `<div style="position:relative;width:160px;height:60px"><button style="position:absolute;left:0;top:0;width:80px;height:30px">확인</button>${control}</div>`,
    );
    expect(inspectOverflow(doc).map((finding) => finding.kind)).toContain(
      "overlapping-controls",
    );
  });
});

describe("protected stress data", () => {
  it("includes the required boundary-sized inputs", () => {
    expect(stressFixtures.company.length).toBe(40);
    expect(stressFixtures.company).not.toMatch(/\s/u);
    expect(stressFixtures.english.length).toBeGreaterThan(40);
    expect(stressFixtures.money).toBe("9876543210123");
    expect(stressFixtures.quantity).toBe("987654321.12345678");
    expect(stressFixtures.probability).toBe("99");
    expect(stressFixtures.holdings).toHaveLength(200);
    expect(stressFixtures.paragraphs).toHaveLength(20);
    expect(stressFixtures.sources).toHaveLength(30);
    expect(stressFixtures.empty).toBe("");
  });
});

describe("editable content around native modals", () => {
  const input =
    '<input id="modal-field" type="text" value="longlonglonglonglonglonglong" style="width:60px">';
  const textarea =
    '<textarea id="modal-field" style="width:120px;height:35px;overflow:auto">one\ntwo\nthree\nfour\nfive</textarea>';
  it.each([
    {
      name: "input outside one modal",
      markup: `${input}<dialog id="first">modal</dialog>`,
      open: ["first"],
      type: "input",
      focus: false,
      report: true,
    },
    {
      name: "textarea outside one modal",
      markup: `${textarea}<dialog id="first">modal</dialog>`,
      open: ["first"],
      type: "textarea",
      focus: false,
      report: true,
    },
    {
      name: "input escaping an inert ancestor in a modal",
      markup: `<div inert><dialog id="first">${input}</dialog></div>`,
      open: ["first"],
      type: "input",
      focus: true,
      report: false,
    },
    {
      name: "textarea escaping an inert ancestor in a modal",
      markup: `<div inert><dialog id="first">${textarea}</dialog></div>`,
      open: ["first"],
      type: "textarea",
      focus: true,
      report: false,
    },
    {
      name: "input escaping a CSS inert ancestor in a modal",
      markup: `<div style="interactivity:inert"><dialog id="first">${input}</dialog></div>`,
      open: ["first"],
      type: "input",
      focus: true,
      report: false,
    },
    {
      name: "input below a later modal",
      markup: `<dialog id="first">${input}</dialog><dialog id="second">upper</dialog>`,
      open: ["first", "second"],
      type: "input",
      focus: false,
      report: true,
    },
    {
      name: "textarea below a modal opened in reverse DOM order",
      markup: `<dialog id="first">upper</dialog><dialog id="second">${textarea}</dialog>`,
      open: ["second", "first"],
      type: "textarea",
      focus: false,
      report: true,
    },
    {
      name: "input in a modal containing another modal",
      markup: `<dialog id="first">${input}<dialog id="second">upper</dialog></dialog>`,
      open: ["first", "second"],
      type: "input",
      focus: false,
      report: true,
    },
    {
      name: "input inside the upper modal",
      markup: `<dialog id="first">lower</dialog><dialog id="second">${input}</dialog>`,
      open: ["first", "second"],
      type: "input",
      focus: true,
      report: false,
    },
    {
      name: "textarea inside the upper modal opened in reverse DOM order",
      markup: `<dialog id="first">${textarea}</dialog><dialog id="second">lower</dialog>`,
      open: ["second", "first"],
      type: "textarea",
      focus: true,
      report: false,
    },
    {
      name: "input in an inert descendant of the modal",
      markup: `<dialog id="first"><div inert>${input}</div></dialog>`,
      open: ["first"],
      type: "input",
      focus: false,
      report: true,
    },
    {
      name: "input in a CSS inert descendant of the modal",
      markup: `<dialog id="first"><div style="interactivity:inert"><div style="interactivity:auto">${input}</div></div></dialog>`,
      open: ["first"],
      type: "input",
      focus: false,
      report: true,
    },
    {
      name: "textarea in an inert modal",
      markup: `<dialog id="first" inert>${textarea}</dialog>`,
      open: ["first"],
      type: "textarea",
      focus: false,
      report: true,
    },
    {
      name: "input with CSS interactivity inert",
      markup: `<div style="interactivity:inert">${input}</div>`,
      open: [],
      type: "input",
      focus: false,
      report: true,
    },
    {
      name: "input with an inert attribute and CSS auto override",
      markup: `<div inert style="interactivity:auto"><div style="interactivity:auto">${input}</div></div>`,
      open: [],
      type: "input",
      focus: false,
      report: true,
    },
    {
      name: "textarea inside CSS inert despite an auto override",
      markup: `<div style="interactivity:inert"><div style="interactivity:auto">${textarea}</div></div>`,
      open: [],
      type: "textarea",
      focus: false,
      report: true,
    },
  ])(
    "classifies $name without changing focus",
    async ({ markup, open, type, focus, report }) => {
      const doc = await makeDocument(markup);
      for (const id of open) {
        const dialog = doc.querySelector<HTMLDialogElement>(`#${id}`);
        if (dialog === null) throw new Error("modal missing");
        dialog.showModal();
      }
      const field = doc.querySelector<HTMLElement>("#modal-field");
      if (field === null) throw new Error("field missing");
      expect(
        field.scrollWidth > field.clientWidth ||
          field.scrollHeight > field.clientHeight,
      ).toBe(true);
      field.focus();
      expect(doc.activeElement === field).toBe(focus);
      const active = doc.activeElement;
      const findings = inspectOverflow(doc);
      expect(doc.activeElement).toBe(active);
      expect(findings).not.toContainEqual({
        kind: "layout-unavailable",
        element: "html",
      });
      expect(
        findings.some(
          (finding) =>
            finding.kind === "unauthorized-overflow" &&
            finding.element === `${type}#modal-field`,
        ),
      ).toBe(report);
    },
  );

  it("does not treat an ordinary open dialog as modal", async () => {
    const doc = await makeDocument(
      `${input}<dialog id="modeless">dialog</dialog>`,
    );
    const dialog = doc.querySelector<HTMLDialogElement>("#modeless");
    const field = doc.querySelector<HTMLInputElement>("#modal-field");
    if (dialog === null || field === null) throw new Error("fixture missing");
    dialog.show();
    field.focus();
    expect(doc.activeElement).toBe(field);
    expect(field.scrollWidth).toBeGreaterThan(field.clientWidth);
    expect(inspectOverflow(doc)).not.toContainEqual({
      kind: "unauthorized-overflow",
      element: "input#modal-field",
    });
  });

  it("detects an inert attribute when CSS interactivity is unavailable", async () => {
    const doc = await makeDocument(`<div inert>${input}</div>`);
    const field = doc.querySelector<HTMLInputElement>("#modal-field");
    if (field === null) throw new Error("field missing");
    field.focus();
    expect(doc.activeElement).not.toBe(field);
    expect(field.scrollWidth).toBeGreaterThan(field.clientWidth);
    const original = globalThis.getComputedStyle;
    const styles = vi
      .spyOn(globalThis, "getComputedStyle")
      .mockImplementation((element, pseudo) => {
        const style = original(element, pseudo);
        return new Proxy(style, {
          get(target, property) {
            if (property === "getPropertyValue")
              return (name: string): string =>
                name === "interactivity" ? "" : target.getPropertyValue(name);
            const value = Reflect.get(target, property, target);
            return typeof value === "function" ? value.bind(target) : value;
          },
        });
      });
    try {
      expect(inspectOverflow(doc)).toContainEqual({
        kind: "unauthorized-overflow",
        element: "input#modal-field",
      });
    } finally {
      styles.mockRestore();
    }
  });

  it("keeps an editable input in a single modal after focus is cleared", async () => {
    const doc = await makeDocument(`<dialog id="first">${input}</dialog>`);
    const dialog = doc.querySelector<HTMLDialogElement>("#first");
    const field = doc.querySelector<HTMLInputElement>("#modal-field");
    if (dialog === null || field === null) throw new Error("fixture missing");
    dialog.showModal();
    field.blur();
    expect(doc.activeElement).toBe(doc.body);
    expect(field.scrollWidth).toBeGreaterThan(field.clientWidth);
    expect(inspectOverflow(doc)).not.toContainEqual({
      kind: "unauthorized-overflow",
      element: "input#modal-field",
    });
    expect(doc.activeElement).toBe(doc.body);
  });

  it.each([
    ["first", "second"],
    ["second", "first"],
  ])(
    "fails closed if multiple modals opened as %s then %s lose focus",
    async (first, second) => {
      const doc = await makeDocument(
        `<label id="modal-label" style="display:block;width:20px;white-space:nowrap">labeltextlabeltext</label><dialog id="first">${input}</dialog><dialog id="second">${textarea}</dialog>`,
      );
      const firstDialog = doc.querySelector<HTMLDialogElement>(`#${first}`);
      const secondDialog = doc.querySelector<HTMLDialogElement>(`#${second}`);
      if (firstDialog === null || secondDialog === null)
        throw new Error("modal missing");
      firstDialog.showModal();
      secondDialog.showModal();
      const active = doc.activeElement as HTMLElement | null;
      if (active === null) throw new Error("focus missing");
      active.blur();
      expect(doc.activeElement).toBe(doc.body);
      const findings = inspectOverflow(doc);
      expect(doc.activeElement).toBe(doc.body);
      expect(findings).toContainEqual({
        kind: "layout-unavailable",
        element: "html",
      });
      expect(findings).toContainEqual({
        kind: "unauthorized-overflow",
        element: "label#modal-label",
      });
      expect(findings).toContainEqual({
        kind: "unauthorized-overflow",
        element: "input#modal-field",
      });
      expect(findings).toContainEqual({
        kind: "unauthorized-overflow",
        element: "textarea#modal-field",
      });
    },
  );

  it.each([
    ["input", input],
    ["textarea", textarea],
  ])("fails closed with only %s content overflow", async (tag, markup) => {
    const doc = await makeDocument(
      `<dialog id="first">${markup}</dialog><dialog id="second">second</dialog>`,
    );
    const first = doc.querySelector<HTMLDialogElement>("#first");
    const second = doc.querySelector<HTMLDialogElement>("#second");
    if (first === null || second === null) throw new Error("modal missing");
    first.showModal();
    second.showModal();
    const active = doc.activeElement as HTMLElement | null;
    if (active === null) throw new Error("focus missing");
    active.blur();
    expect(doc.activeElement).toBe(doc.body);
    const findings = inspectOverflow(doc);
    expect(findings).toContainEqual({
      kind: "layout-unavailable",
      element: "html",
    });
    expect(findings).toContainEqual({
      kind: "unauthorized-overflow",
      element: `${tag}#modal-field`,
    });
    expect(doc.activeElement).toBe(doc.body);
  });

  it("does not reject an unresolved modal stack without overflowing controls", async () => {
    const doc = await makeDocument(
      '<input id="small-field" value="ok" style="width:120px"><dialog id="first">first</dialog><dialog id="second">second</dialog>',
    );
    const first = doc.querySelector<HTMLDialogElement>("#first");
    const second = doc.querySelector<HTMLDialogElement>("#second");
    const field = doc.querySelector<HTMLInputElement>("#small-field");
    if (first === null || second === null || field === null)
      throw new Error("fixture missing");
    expect(field.scrollWidth).toBe(field.clientWidth);
    first.showModal();
    second.showModal();
    const active = doc.activeElement as HTMLElement | null;
    if (active === null) throw new Error("focus missing");
    active.blur();
    expect(doc.activeElement).toBe(doc.body);
    expect(inspectOverflow(doc)).not.toContainEqual({
      kind: "layout-unavailable",
      element: "html",
    });
  });
});

describe("font metrics and clipping boundaries", () => {
  it.each([
    ["mask-image", "linear-gradient(transparent,transparent)"],
    ["-webkit-mask-image", "linear-gradient(#000 50%,transparent 50%)"],
  ])("rejects a metric hidden by its own %s", async (property, value) => {
    const doc = await makeDocument(
      `<div style="padding-top:20px"><p id="masked-metric" style="font:12px/16px IBM Plex Sans KR;margin:0;${property}:${value}">주식 코파일럿</p></div>`,
    );
    await loadProbeFont(doc);
    const paragraph = doc.querySelector<HTMLElement>("#masked-metric");
    if (paragraph === null) throw new Error("metric missing");
    expect(paragraph.scrollHeight - paragraph.clientHeight).toBe(1);
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "unauthorized-overflow",
      element: "p#masked-metric",
    });
  });

  it.each([
    ["mask-image", "linear-gradient(transparent,transparent)"],
    ["-webkit-mask-image", "linear-gradient(#000 50%,transparent 50%)"],
  ])("rejects a metric hidden by its parent's %s", async (property, value) => {
    const doc = await makeDocument(
      `<div style="padding-top:20px"><div id="masked-parent" style="height:16px;${property}:${value}"><p id="masked-child" style="font:12px/16px IBM Plex Sans KR;margin:0">주식 코파일럿</p></div></div>`,
    );
    await loadProbeFont(doc);
    const parent = doc.querySelector<HTMLElement>("#masked-parent");
    if (parent === null) throw new Error("parent missing");
    expect(parent.scrollHeight - parent.clientHeight).toBe(1);
    const findings = inspectOverflow(doc);
    expect(findings).toContainEqual({
      kind: "unauthorized-overflow",
      element: "div#masked-parent",
    });
    expect(findings).toContainEqual({
      kind: "unauthorized-overflow",
      element: "p#masked-child",
    });
  });

  it.each([
    { owner: "self", expected: ["p#box-metric"] },
    { owner: "parent", expected: ["div#box-parent", "p#box-metric"] },
  ])(
    "rejects a metric hidden by a box mask on $owner",
    async ({ owner, expected }) => {
      const mask =
        "-webkit-mask-box-image:linear-gradient(transparent,transparent) 0 fill / 0 / 0 stretch";
      const doc = await makeDocument(
        `<div style="padding-top:20px"><div id="box-parent" style="height:16px;${owner === "parent" ? mask : ""}"><p id="box-metric" style="font:12px/16px IBM Plex Sans KR;margin:0;${owner === "self" ? mask : ""}">주식 코파일럿</p></div></div>`,
      );
      await loadProbeFont(doc);
      const metric = doc.querySelector<HTMLElement>("#box-metric");
      const parent = doc.querySelector<HTMLElement>("#box-parent");
      if (metric === null || parent === null) throw new Error("metric missing");
      const style = getComputedStyle(owner === "self" ? metric : parent);
      expect(style.getPropertyValue("mask-image")).toBe("none");
      expect(style.getPropertyValue("-webkit-mask-box-image-source")).not.toBe(
        "none",
      );
      expect(style.getPropertyValue("-webkit-mask-box-image-source")).not.toBe(
        "",
      );
      expect(metric.scrollHeight - metric.clientHeight).toBe(1);
      const findings = inspectOverflow(doc);
      for (const element of expected)
        expect(findings).toContainEqual({
          kind: "unauthorized-overflow",
          element,
        });
    },
  );

  it("detects a real overflowing sibling beside a metric child", async () => {
    const doc = await makeDocument(
      '<div style="padding-top:20px"><div id="metric-with-sibling" style="position:relative;height:16px"><p style="font:12px/16px IBM Plex Sans KR;margin:0">주식 코파일럿</p><div style="position:absolute;top:16px;width:1px;height:1px"></div></div></div>',
    );
    await loadProbeFont(doc);
    const parent = doc.querySelector<HTMLElement>("#metric-with-sibling");
    if (parent === null) throw new Error("parent missing");
    expect(parent.scrollHeight - parent.clientHeight).toBe(1);
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "unauthorized-overflow",
      element: "div#metric-with-sibling",
    });
  });
});
describe("unresolved modal order and already known findings", () => {
  it.each([
    [
      "readonly input",
      '<input readonly id="known-field" value="longlonglonglonglonglong" style="width:60px">',
      "input#known-field",
    ],
    [
      "disabled input",
      '<input disabled id="known-field" value="longlonglonglonglonglong" style="width:60px">',
      "input#known-field",
    ],
    [
      "textarea with hidden vertical overflow",
      '<textarea id="known-field" style="width:120px;height:35px;overflow:hidden">one\ntwo\nthree\nfour\nfive</textarea>',
      "textarea#known-field",
    ],
    [
      "textarea with only horizontal overflow",
      '<textarea id="known-field" wrap="off" style="width:60px;height:80px;overflow:auto">longlonglonglonglonglonglonglong</textarea>',
      "textarea#known-field",
    ],
  ])("does not mark %s unavailable", async (_name, markup, element) => {
    const doc = await makeDocument(
      `${markup}<dialog id="first">first</dialog><dialog id="second">second</dialog>`,
    );
    const first = doc.querySelector<HTMLDialogElement>("#first");
    const second = doc.querySelector<HTMLDialogElement>("#second");
    const field = doc.querySelector<HTMLElement>("#known-field");
    if (first === null || second === null || field === null)
      throw new Error("fixture missing");
    first.showModal();
    second.showModal();
    const active = doc.activeElement as HTMLElement | null;
    if (active === null) throw new Error("focus missing");
    active.blur();
    expect(doc.activeElement).toBe(doc.body);
    expect(
      field.scrollWidth > field.clientWidth ||
        field.scrollHeight > field.clientHeight,
    ).toBe(true);
    const findings = inspectOverflow(doc);
    expect(findings).toContainEqual({ kind: "unauthorized-overflow", element });
    expect(findings).not.toContainEqual({
      kind: "layout-unavailable",
      element: "html",
    });
  });
});

describe("non-editable native textarea", () => {
  it.each(["readonly", "disabled"])(
    "reports vertical scrolling on a %s textarea",
    async (state) => {
      const doc = await makeDocument(
        `<textarea id="locked-area" ${state} style="width:120px;height:35px;overflow:auto">one\ntwo\nthree\nfour\nfive</textarea>`,
      );
      const area = doc.querySelector<HTMLTextAreaElement>("#locked-area");
      if (area === null) throw new Error("textarea missing");
      expect(area.scrollHeight).toBeGreaterThan(area.clientHeight);
      expect(inspectOverflow(doc)).toContainEqual({
        kind: "unauthorized-overflow",
        element: "textarea#locked-area",
      });
    },
  );
});

const catalogStory = async (
  component: string,
  exportName: string,
  width: "normal" | "narrow",
  scale: "100%" | "125%",
): Promise<{ doc: Document; slot: HTMLElement }> => {
  const { doc, slot } = await narrowStory(component, exportName);
  slot.style.boxSizing = width === "narrow" ? "content-box" : "";
  slot.style.width = width === "narrow" ? "126px" : "";
  doc.documentElement.style.fontSize = scale;
  const view = doc.defaultView;
  if (view === null) throw new Error(`${component} view missing`);
  await new Promise<void>((resolve) =>
    view.requestAnimationFrame(() =>
      view.requestAnimationFrame(() => resolve()),
    ),
  );
  return { doc, slot };
};

const singleLine = (doc: Document, element: Element): boolean => {
  const range = doc.createRange();
  range.selectNodeContents(element);
  const tops = [...range.getClientRects()]
    .filter((rect) => rect.width > 0 && rect.height > 0)
    .map((rect) => Math.round(rect.top));
  return tops.length > 0 && new Set(tops).size === 1;
};

describe("DS-23 layout, numeric preservation, and scroll area guarantees", () => {
  it.each([
    ["Money", "Light0"],
    ["Percent", "Light4"],
    ["Quantity", "Light1"],
    ["PriceChange", "Light0"],
    ["Card", "Light0"],
    ["Table", "Light0"],
    ["DonutChart", "Light0"],
  ])(
    "keeps %s numbers on a single unclipped line inside a horizontal scroll area",
    async (component, story) => {
      const { doc } = await catalogStory(component, story, "narrow", "125%");
      const numbers = [
        ...doc.querySelectorAll<HTMLElement>(".tabular-nums.whitespace-nowrap"),
      ].filter((el) => el.closest("table.sr-only") === null);
      expect(numbers.length).toBeGreaterThan(0);
      for (const number of numbers) {
        const style = getComputedStyle(number);
        expect(style.whiteSpace).toBe("nowrap");
        expect(style.textOverflow).not.toBe("ellipsis");
        expect(singleLine(doc, number)).toBe(true);
        const viewport = number.closest<HTMLElement>("[data-scroll-viewport]");
        expect(viewport).not.toBeNull();
        if (viewport === null) throw new Error("horizontal viewport missing");
        const viewportStyle = getComputedStyle(viewport);
        expect(viewportStyle.overflowX).toBe("auto");
        expect(viewportStyle.overflowY).toBe("hidden");
      }
      expect(inspectOverflow(doc)).toEqual([]);
    },
    20000,
  );

  it("shows horizontal scrollbar and keyboard focus only when content overflows", async () => {
    const narrow = await catalogStory("Money", "Light0", "narrow", "100%");
    const narrowViewport = narrow.doc.querySelector<HTMLElement>(
      ".scroll-viewport-x[data-scroll-viewport]",
    );
    if (narrowViewport === null) throw new Error("narrow viewport missing");
    expect(narrowViewport.scrollWidth).toBeGreaterThan(
      narrowViewport.clientWidth,
    );
    expect(narrowViewport.tabIndex).toBe(0);
    narrowViewport.focus();
    expect(narrow.doc.activeElement).toBe(narrowViewport);
    narrowViewport.scrollLeft = narrowViewport.scrollWidth;
    expect(narrowViewport.scrollLeft).toBeGreaterThan(0);
    const narrowBar = narrow.doc.querySelector<HTMLElement>(
      '[data-orientation="horizontal"]',
    );
    expect(narrowBar).not.toBeNull();
    if (narrowBar !== null)
      expect(getComputedStyle(narrowBar).position).toBe("absolute");

    const normal = await catalogStory("Money", "Light0", "normal", "100%");
    const normalViewport = normal.doc.querySelector<HTMLElement>(
      ".scroll-viewport-x[data-scroll-viewport]",
    );
    if (normalViewport === null) throw new Error("normal viewport missing");
    expect(normalViewport.scrollWidth).toBeLessThanOrEqual(
      normalViewport.clientWidth,
    );
    expect(normalViewport.tabIndex).toBe(-1);
    expect(
      normal.doc.querySelector('[data-orientation="horizontal"]'),
    ).toBeNull();
  }, 20000);

  it("shows vertical stage scrollbar and keyboard focus only when tall, without shrinking content width", async () => {
    const tall = await catalogStory("Markdown", "Light2", "normal", "100%");
    const tallViewport = tall.doc.querySelector<HTMLElement>(
      ".scroll-viewport-y[data-scroll-viewport]",
    );
    const tallContent = tall.doc.querySelector<HTMLElement>(
      "[data-catalog-content]",
    );
    if (tallViewport === null || tallContent === null)
      throw new Error("tall stage missing");
    expect(tallViewport.scrollHeight).toBeGreaterThan(
      tallViewport.clientHeight,
    );
    expect(tallViewport.tabIndex).toBe(0);
    tallViewport.focus();
    expect(tall.doc.activeElement).toBe(tallViewport);
    tallViewport.scrollTop = 40;
    expect(tallViewport.scrollTop).toBeGreaterThan(0);
    const tallBar = tall.doc.querySelector<HTMLElement>(
      '[data-orientation="vertical"]',
    );
    expect(tallBar).not.toBeNull();
    if (tallBar !== null)
      expect(getComputedStyle(tallBar).position).toBe("absolute");

    const short = await catalogStory("Button", "Light0", "normal", "100%");
    const shortViewport = short.doc.querySelector<HTMLElement>(
      ".scroll-viewport-y[data-scroll-viewport]",
    );
    const shortContent = short.doc.querySelector<HTMLElement>(
      "[data-catalog-content]",
    );
    if (shortViewport === null || shortContent === null)
      throw new Error("short stage missing");
    expect(shortViewport.scrollHeight).toBeLessThanOrEqual(
      shortViewport.clientHeight,
    );
    expect(shortViewport.tabIndex).toBe(-1);
    expect(short.doc.querySelector('[data-orientation="vertical"]')).toBeNull();
    expect(tallContent.clientWidth).toBe(shortContent.clientWidth);
  }, 20000);

  it("still reports real horizontal overflow and nested vertical scroll inside the catalog stage", async () => {
    const { doc } = await catalogStory("Button", "Light0", "narrow", "100%");
    const content = doc.querySelector<HTMLElement>("[data-catalog-content]");
    if (content === null) throw new Error("catalog content missing");

    const badText = doc.createElement("div");
    badText.id = "ds23-bad-overflow";
    badText.style.width = "40px";
    badText.style.whiteSpace = "nowrap";
    badText.textContent = "verylongunbreakabletextverylongunbreakabletext";
    content.append(badText);
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "unauthorized-overflow",
      element: "div#ds23-bad-overflow",
    });
    badText.remove();

    const wideBox = doc.createElement("div");
    wideBox.id = "ds23-wide-box";
    wideBox.style.width = "240px";
    wideBox.style.height = "20px";
    wideBox.textContent = "wide";
    content.append(wideBox);
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "clipped",
      element: "div#ds23-wide-box",
    });
    wideBox.remove();

    const nested = doc.createElement("div");
    nested.id = "ds23-nested-scroll";
    nested.setAttribute("data-scroll-viewport", "");
    nested.style.height = "80px";
    nested.style.overflowY = "auto";
    nested.innerHTML = '<div style="height:200px">nested</div>';
    content.append(nested);
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "nested-scroll",
      element: "div#ds23-nested-scroll",
    });
    nested.remove();
  }, 20000);

  it("wraps secret TextField paste button below the input only when narrow", async () => {
    const normal = await catalogStory("TextField", "Light4", "normal", "100%");
    const normalInput = normal.slot.querySelector<HTMLElement>("input");
    const normalButtons = [
      ...normal.slot.querySelectorAll<HTMLElement>("button"),
    ];
    const normalPaste = normalButtons.at(-1);
    if (normalInput === null || normalPaste === undefined)
      throw new Error("normal secret field missing");
    const normalInputRect = normalInput.getBoundingClientRect();
    const normalPasteRect = normalPaste.getBoundingClientRect();
    expect(Math.abs(normalPasteRect.top - normalInputRect.top)).toBeLessThan(
      normalInputRect.height,
    );
    expect(inspectOverflow(normal.doc)).toEqual([]);

    const narrow = await catalogStory("TextField", "Light4", "narrow", "100%");
    const narrowInput = narrow.slot.querySelector<HTMLElement>("input");
    const narrowButtons = [
      ...narrow.slot.querySelectorAll<HTMLElement>("button"),
    ];
    const narrowPaste = narrowButtons.at(-1);
    if (narrowInput === null || narrowPaste === undefined)
      throw new Error("narrow secret field missing");
    expect(narrowPaste.getBoundingClientRect().top).toBeGreaterThanOrEqual(
      narrowInput.getBoundingClientRect().bottom,
    );
    expect(inspectOverflow(narrow.doc)).toEqual([]);
  }, 20000);

  it("scrolls Dialog body while keeping header and footer fixed", async () => {
    const { doc } = await catalogStory("Dialog", "Light4", "narrow", "125%");
    const dialog = doc.querySelector<HTMLElement>('[role="dialog"]');
    const header = dialog?.querySelector<HTMLElement>("header");
    const footer = dialog?.querySelector<HTMLElement>("footer");
    const body = dialog?.querySelector<HTMLElement>("[data-scroll-viewport]");
    if (dialog === null || header == null || footer == null || body == null)
      throw new Error("dialog parts missing");
    expect(body.contains(header)).toBe(false);
    expect(body.contains(footer)).toBe(false);
    expect(getComputedStyle(body).overflowY).toBe("auto");
    expect(inspectOverflow(doc)).toEqual([]);

    const spacer = doc.createElement("div");
    spacer.style.height = "900px";
    spacer.textContent = "긴 대화 본문";
    body.firstElementChild?.append(spacer);
    const view = doc.defaultView;
    if (view === null) throw new Error("dialog view missing");
    await new Promise<void>((resolve) =>
      view.requestAnimationFrame(() =>
        view.requestAnimationFrame(() => resolve()),
      ),
    );
    expect(body.scrollHeight).toBeGreaterThan(body.clientHeight);
    expect(body.tabIndex).toBe(0);
    const headerBefore = header.getBoundingClientRect().top;
    const footerBefore = footer.getBoundingClientRect().top;
    body.scrollTop = 120;
    expect(body.scrollTop).toBeGreaterThan(0);
    expect(header.getBoundingClientRect().top).toBe(headerBefore);
    expect(footer.getBoundingClientRect().top).toBe(footerBefore);
    expect(inspectOverflow(doc)).toEqual([]);
  }, 20000);

  it("expands Button height, wraps StatusBar action, and keeps Checkbox mark inside its border", async () => {
    const buttonView = await catalogStory(
      "Button",
      "Light12",
      "narrow",
      "100%",
    );
    const button = buttonView.slot.querySelector<HTMLElement>("button");
    if (button === null) throw new Error("button missing");
    expect(button.getBoundingClientRect().height).toBeGreaterThan(36);
    expect(inspectOverflow(buttonView.doc)).toEqual([]);

    const statusView = await catalogStory(
      "StatusBar",
      "Light10",
      "narrow",
      "125%",
    );
    const statusText =
      statusView.slot.querySelector<HTMLElement>('[role="status"]');
    const statusAction = statusView.slot.querySelector<HTMLElement>("button");
    if (statusText === null || statusAction === null)
      throw new Error("status bar missing");
    expect(statusAction.getBoundingClientRect().top).toBeGreaterThanOrEqual(
      statusText.getBoundingClientRect().bottom,
    );
    expect(inspectOverflow(statusView.doc)).toEqual([]);

    const checkboxView = await catalogStory(
      "Checkbox",
      "Light2",
      "narrow",
      "100%",
    );
    const box = checkboxView.slot.querySelector<HTMLElement>(
      'button[role="checkbox"]',
    );
    const svg = box?.querySelector<SVGElement>("svg");
    if (box === null || svg == null) throw new Error("checkbox missing");
    const boxRect = box.getBoundingClientRect();
    const svgRect = svg.getBoundingClientRect();
    expect(Math.round(boxRect.width)).toBe(16);
    expect(Math.round(boxRect.height)).toBe(16);
    expect(svgRect.left).toBeGreaterThanOrEqual(boxRect.left);
    expect(svgRect.right).toBeLessThanOrEqual(boxRect.right);
    expect(svgRect.top).toBeGreaterThanOrEqual(boxRect.top);
    expect(svgRect.bottom).toBeLessThanOrEqual(boxRect.bottom);
    expect(inspectOverflow(checkboxView.doc)).toEqual([]);

    for (const [component, story] of [
      ["Badge", "Light6"],
      ["SegmentedControl", "Light3"],
      ["Disclosure", "Light2"],
    ] as const) {
      const view = await catalogStory(component, story, "narrow", "125%");
      expect(inspectOverflow(view.doc)).toEqual([]);
    }
  }, 20000);

  it("keeps focused LineChart status number scrollable by keyboard and unclipped", async () => {
    const { doc, slot } = await catalogStory(
      "LineChart",
      "Light6",
      "narrow",
      "125%",
    );
    const explore = slot.querySelector<HTMLElement>("button");
    if (explore === null) throw new Error("chart explore button missing");
    explore.focus();
    const view = doc.defaultView;
    if (view === null) throw new Error("chart view missing");
    const flush = (): Promise<void> =>
      new Promise<void>((resolve) =>
        view.requestAnimationFrame(() =>
          view.requestAnimationFrame(() => resolve()),
        ),
      );
    await flush();
    const status = slot.querySelector<HTMLElement>('[role="status"]');
    const number = status?.querySelector<HTMLElement>(
      ".tabular-nums.whitespace-nowrap",
    );
    if (status === null || number == null)
      throw new Error("chart status number missing");
    expect(singleLine(doc, number)).toBe(true);
    const viewport = number.closest<HTMLElement>("[data-scroll-viewport]");
    expect(viewport).not.toBeNull();
    if (viewport === null) throw new Error("chart status viewport missing");
    expect(getComputedStyle(viewport).overflowX).toBe("auto");
    expect(getComputedStyle(viewport).overflowY).toBe("hidden");
    expect(viewport.scrollWidth).toBeGreaterThan(viewport.clientWidth);
    expect(viewport.tabIndex).toBe(0);
    expect(inspectOverflow(doc)).toEqual([]);

    viewport.focus();
    await flush();
    expect(doc.activeElement).toBe(viewport);
    expect(status.textContent).not.toBe("");
    expect(viewport.scrollWidth).toBeGreaterThan(viewport.clientWidth);
    expect(viewport.tabIndex).toBe(0);
    viewport.scrollLeft = 40;
    expect(viewport.scrollLeft).toBeGreaterThan(0);

    viewport.blur();
    await flush();
    expect(status.textContent).toBe("");
  }, 20000);

  it("updates ScrollArea scrollbar and tabIndex when content grows without viewport resize", async () => {
    const card = await catalogStory("Card", "Light0", "normal", "100%");
    card.slot.style.width = "300px";
    const cardView = card.doc.defaultView;
    if (cardView === null) throw new Error("card view missing");
    const flush = (win: Window): Promise<void> =>
      new Promise<void>((resolve) =>
        win.requestAnimationFrame(() =>
          win.requestAnimationFrame(() => resolve()),
        ),
      );
    await flush(cardView);
    const cardVp = card.slot.querySelector<HTMLElement>(
      ".scroll-viewport-x[data-scroll-viewport]",
    );
    if (cardVp === null) throw new Error("card viewport missing");
    expect(cardVp.scrollWidth).toBeLessThanOrEqual(cardVp.clientWidth);
    expect(cardVp.tabIndex).toBe(-1);
    expect(
      cardVp.parentElement?.querySelector('[data-orientation="horizontal"]'),
    ).toBeNull();
    const widthBefore = cardVp.clientWidth;
    const wide = card.doc.createElement("span");
    wide.className = "tabular-nums whitespace-nowrap w-max";
    wide.textContent = "9,876,543,210,987,654,321,098,765,432,109,876원";
    cardVp.firstElementChild?.firstElementChild?.append(wide);
    await flush(cardView);
    expect(cardVp.clientWidth).toBe(widthBefore);
    expect(cardVp.scrollWidth).toBeGreaterThan(cardVp.clientWidth);
    expect(cardVp.tabIndex).toBe(0);
    expect(
      cardVp.parentElement?.querySelector('[data-orientation="horizontal"]'),
    ).not.toBeNull();

    const stage = await catalogStory("Button", "Light0", "normal", "100%");
    const stageVp = stage.doc.querySelector<HTMLElement>(
      ".scroll-viewport-y[data-scroll-viewport]",
    );
    const stageContent = stage.doc.querySelector<HTMLElement>(
      "[data-catalog-content]",
    );
    const stageView = stage.doc.defaultView;
    if (stageVp === null || stageContent === null || stageView === null)
      throw new Error("stage viewport missing");
    const heightBefore = stageVp.clientHeight;
    expect(stageVp.tabIndex).toBe(-1);
    const tall = stage.doc.createElement("div");
    tall.style.height = "1200px";
    tall.textContent = "긴 동적 본문";
    stageContent.append(tall);
    await flush(stageView);
    expect(stageVp.clientHeight).toBe(heightBefore);
    expect(stageVp.scrollHeight).toBeGreaterThan(stageVp.clientHeight);
    expect(stageVp.tabIndex).toBe(0);
    expect(
      stageVp.parentElement?.querySelector('[data-orientation="vertical"]'),
    ).not.toBeNull();
  }, 20000);

  it("allows horizontal scroll without clipping when Dialog body contains a long number", async () => {
    const { doc } = await catalogStory("Dialog", "Light0", "narrow", "125%");
    const frame = doc.defaultView?.frameElement;
    if (frame instanceof HTMLIFrameElement) frame.width = "320";
    const view = doc.defaultView;
    if (view === null) throw new Error("dialog view missing");
    const flush = (): Promise<void> =>
      new Promise<void>((resolve) =>
        view.requestAnimationFrame(() =>
          view.requestAnimationFrame(() => resolve()),
        ),
      );
    await flush();
    const dialog = doc.querySelector<HTMLElement>('[role="dialog"]');
    const body = dialog?.querySelector<HTMLElement>("[data-scroll-viewport]");
    if (dialog === null || body == null) throw new Error("dialog body missing");
    expect(getComputedStyle(body).overflowX).toBe("auto");
    expect(getComputedStyle(body).overflowY).toBe("auto");
    const amount = doc.createElement("span");
    amount.className = "tabular-nums whitespace-nowrap w-max";
    amount.textContent = "9,876,543,210,987,654,321원";
    body.firstElementChild?.firstElementChild?.append(amount);
    await flush();
    expect(singleLine(doc, amount)).toBe(true);
    expect(body.scrollWidth).toBeGreaterThan(body.clientWidth);
    expect(body.tabIndex).toBe(0);
    expect(
      body.parentElement?.querySelector('[data-orientation="horizontal"]'),
    ).not.toBeNull();
    expect(inspectOverflow(doc)).toEqual([]);
    body.scrollLeft = 40;
    expect(body.scrollLeft).toBeGreaterThan(0);
  }, 20000);

  it("keeps unidirectional vertical ScrollArea unfocusable on hidden-axis overflow while still reporting clipped", async () => {
    const { doc } = await catalogStory("Button", "Light0", "normal", "100%");
    const viewport = doc.querySelector<HTMLElement>(
      ".scroll-viewport-y[data-scroll-viewport]",
    );
    const content = doc.querySelector<HTMLElement>("[data-catalog-content]");
    const view = doc.defaultView;
    if (viewport === null || content === null || view === null)
      throw new Error("stage missing");
    expect(viewport.scrollHeight).toBeLessThanOrEqual(viewport.clientHeight);
    expect(viewport.tabIndex).toBe(-1);
    const wide = doc.createElement("div");
    wide.id = "ds23-hidden-axis-wide";
    wide.style.width = "1600px";
    wide.style.height = "20px";
    wide.textContent = "hidden-axis-wide";
    content.append(wide);
    await new Promise<void>((resolve) =>
      view.requestAnimationFrame(() =>
        view.requestAnimationFrame(() => resolve()),
      ),
    );
    expect(viewport.scrollWidth).toBeLessThanOrEqual(viewport.clientWidth);
    expect(viewport.tabIndex).toBe(-1);
    expect(inspectOverflow(doc)).toContainEqual({
      kind: "clipped",
      element: "div#ds23-hidden-axis-wide",
    });
    wide.remove();
  }, 20000);

  it("right-aligns ChartAxes maximum at wide container width and keeps horizontal scrollbar clear of text", async () => {
    const wide = await catalogStory("LineChart", "Light0", "normal", "100%");
    const frame = wide.doc.defaultView?.frameElement;
    if (frame instanceof HTMLIFrameElement) frame.width = "1100";
    const view = wide.doc.defaultView;
    if (view === null) throw new Error("wide chart view missing");
    await new Promise<void>((resolve) =>
      view.requestAnimationFrame(() =>
        view.requestAnimationFrame(() => resolve()),
      ),
    );
    const spans = [...wide.slot.querySelectorAll<HTMLElement>("span")];
    const lastDate = spans.find((el) => el.textContent?.includes("11월 13일"));
    const maxAxis = spans.find((el) => el.textContent?.includes("최대"));
    if (lastDate === undefined || maxAxis === undefined)
      throw new Error("chart axis labels missing");
    expect(
      Math.abs(
        maxAxis.getBoundingClientRect().right -
          lastDate.getBoundingClientRect().right,
      ),
    ).toBeLessThanOrEqual(4);

    const money = await catalogStory("Money", "Light0", "narrow", "100%");
    const number = money.slot.querySelector<HTMLElement>(
      ".tabular-nums.whitespace-nowrap",
    );
    const bar = money.slot.querySelector<HTMLElement>(
      '[data-orientation="horizontal"]',
    );
    if (number === null || bar === null)
      throw new Error("money scrollbar missing");
    expect(bar.getBoundingClientRect().top).toBeGreaterThanOrEqual(
      number.getBoundingClientRect().bottom - 1,
    );
  }, 20000);

  it("keeps StatusBar icon beside its message at narrow widths while wrapping the action below", async () => {
    const { doc, slot } = await catalogStory(
      "StatusBar",
      "Light10",
      "narrow",
      "100%",
    );
    slot.style.width = "180px";
    const view = doc.defaultView;
    if (view === null) throw new Error("status view missing");
    await new Promise<void>((resolve) =>
      view.requestAnimationFrame(() =>
        view.requestAnimationFrame(() => resolve()),
      ),
    );
    const icon = slot.querySelector<SVGElement>("svg");
    const status = slot.querySelector<HTMLElement>('[role="status"]');
    const button = slot.querySelector<HTMLElement>("button");
    if (icon === null || status === null || button === null)
      throw new Error("status parts missing");
    expect(icon.getBoundingClientRect().bottom).toBeGreaterThan(
      status.getBoundingClientRect().top,
    );
    expect(button.getBoundingClientRect().top).toBeGreaterThanOrEqual(
      status.getBoundingClientRect().bottom,
    );
    expect(inspectOverflow(doc)).toEqual([]);
  }, 20000);

  it("lets Card horizontal ScrollArea absorb nested wrappers with long numbers without double horizontal scroll", async () => {
    const { doc, slot } = await catalogStory(
      "Card",
      "Light0",
      "narrow",
      "100%",
    );
    const cardVp = slot.querySelector<HTMLElement>(
      ".scroll-viewport-x[data-scroll-viewport]",
    );
    const view = doc.defaultView;
    if (cardVp === null || view === null) throw new Error("card missing");
    const flush = (): Promise<void> =>
      new Promise<void>((resolve) =>
        view.requestAnimationFrame(() =>
          view.requestAnimationFrame(() => resolve()),
        ),
      );
    const nestedWrap = doc.createElement("div");
    nestedWrap.className = "flex flex-col gap-space-2";
    const amount = doc.createElement("span");
    amount.className = "tabular-nums whitespace-nowrap w-max";
    amount.textContent = "9,876,543,210,987원";
    nestedWrap.append(amount);
    cardVp.firstElementChild?.firstElementChild?.append(nestedWrap);
    await flush();
    expect(nestedWrap.scrollWidth).toBeLessThanOrEqual(nestedWrap.clientWidth);
    expect(cardVp.scrollWidth).toBeGreaterThan(cardVp.clientWidth);
    expect(cardVp.tabIndex).toBe(0);
    expect(inspectOverflow(doc)).toEqual([]);
    nestedWrap.remove();
    await flush();

    const innerScroll = doc.createElement("div");
    innerScroll.setAttribute("data-scroll-viewport", "");
    innerScroll.className = "scroll-viewport-x w-full max-w-full scroll-flex";
    const innerAmount = doc.createElement("span");
    innerAmount.className = "block tabular-nums whitespace-nowrap w-max";
    innerAmount.textContent = "9,876,543,210,987,654,321원";
    innerScroll.append(innerAmount);
    cardVp.firstElementChild?.firstElementChild?.append(innerScroll);
    await flush();
    expect(cardVp.scrollWidth).toBeGreaterThan(cardVp.clientWidth);
    expect(innerScroll.scrollWidth).toBeLessThanOrEqual(
      innerScroll.clientWidth,
    );
    expect(inspectOverflow(doc)).toEqual([]);
  }, 20000);
});

describe("DS-11 layout components, container-query bands, and child min-width guarantees", () => {
  it("switches GridItem column spans by container width across narrow, medium, and wide bands", async () => {
    const { doc, slot } = await catalogStory(
      "Grid",
      "Light0",
      "normal",
      "100%",
    );
    const frame = doc.defaultView?.frameElement;
    const view = doc.defaultView;
    if (!(frame instanceof HTMLIFrameElement) || view === null)
      throw new Error("grid frame missing");
    const flush = (): Promise<void> =>
      new Promise<void>((resolve) =>
        view.requestAnimationFrame(() =>
          view.requestAnimationFrame(() => resolve()),
        ),
      );
    const items = (): HTMLElement[] => {
      const grid = slot.querySelector<HTMLElement>(".grid-cols-layout");
      if (grid === null) throw new Error("grid container missing");
      return [...grid.children] as HTMLElement[];
    };
    const setContainerWidth = async (widthPx: number): Promise<void> => {
      frame.width = `${widthPx + 120}`;
      slot.style.boxSizing = "content-box";
      slot.style.width = `${widthPx}px`;
      await flush();
    };

    await setContainerWidth(640);
    const narrowItems = items();
    expect(narrowItems).toHaveLength(3);
    expect(getComputedStyle(narrowItems[0]!).gridColumnStart).toBe("span 12");
    expect(
      Math.round(narrowItems[1]!.getBoundingClientRect().top),
    ).toBeGreaterThan(Math.round(narrowItems[0]!.getBoundingClientRect().top));
    expect(inspectOverflow(doc)).toEqual([]);

    await setContainerWidth(900);
    const mediumItems = items();
    expect(getComputedStyle(mediumItems[0]!).gridColumnStart).toBe("span 6");
    expect(getComputedStyle(mediumItems[1]!).gridColumnStart).toBe("span 6");
    expect(getComputedStyle(mediumItems[2]!).gridColumnStart).toBe("span 12");
    expect(
      Math.abs(
        mediumItems[0]!.getBoundingClientRect().top -
          mediumItems[1]!.getBoundingClientRect().top,
      ),
    ).toBeLessThanOrEqual(1);
    expect(
      Math.round(mediumItems[2]!.getBoundingClientRect().top),
    ).toBeGreaterThan(Math.round(mediumItems[0]!.getBoundingClientRect().top));
    expect(inspectOverflow(doc)).toEqual([]);

    await setContainerWidth(1200);
    const wideItems = items();
    expect(getComputedStyle(wideItems[0]!).gridColumnStart).toBe("span 4");
    expect(getComputedStyle(wideItems[1]!).gridColumnStart).toBe("span 4");
    expect(getComputedStyle(wideItems[2]!).gridColumnStart).toBe("span 4");
    expect(
      Math.abs(
        wideItems[0]!.getBoundingClientRect().top -
          wideItems[2]!.getBoundingClientRect().top,
      ),
    ).toBeLessThanOrEqual(1);
    expect(inspectOverflow(doc)).toEqual([]);
  }, 20000);

  it.each([
    ["Stack", "Light5"],
    ["Inline", "Light3"],
    ["Inline", "Light4"],
    ["Split", "Light5"],
    ["GridItem", "Light7"],
  ])(
    "keeps %s (%s) child min-width:0 wrappers free of overflow with long unbroken text",
    async (component, story) => {
      for (const width of ["normal", "narrow"] as const) {
        const { doc, slot } = await catalogStory(
          component,
          story,
          width,
          "125%",
        );
        const wrappers = [
          ...slot.querySelectorAll<HTMLElement>(".scroll-flex"),
        ];
        expect(wrappers.length).toBeGreaterThan(0);
        for (const wrapper of wrappers) {
          const style = getComputedStyle(wrapper);
          if (!wrapper.classList.contains("wrap-slot"))
            expect(style.minWidth).toBe("0px");
          expect(style.minHeight).toBe("0px");
        }
        expect(inspectOverflow(doc)).toEqual([]);
      }
    },
    20000,
  );

  it.each([
    ["Light0", 232],
    ["Light1", 56],
    ["Light2", 384],
    ["Light3", 320],
  ])(
    "sizes Split fixed pane (%s) to %dpx at wide width and wraps cleanly at narrow width",
    async (story, expectedPx) => {
      const wide = await catalogStory("Split", story, "normal", "100%");
      const splitWide = wide.slot.querySelector<HTMLElement>(
        "[data-catalog-content] > div",
      );
      if (splitWide === null) throw new Error("split missing");
      const fixedWide = splitWide.children[0] as HTMLElement;
      const flexWide = splitWide.children[1] as HTMLElement;
      expect(Math.round(fixedWide.getBoundingClientRect().width)).toBe(
        expectedPx,
      );
      expect(
        Math.abs(
          fixedWide.getBoundingClientRect().top -
            flexWide.getBoundingClientRect().top,
        ),
      ).toBeLessThanOrEqual(1);
      expect(inspectOverflow(wide.doc)).toEqual([]);

      const narrow = await catalogStory("Split", story, "narrow", "125%");
      expect(inspectOverflow(narrow.doc)).toEqual([]);
    },
    20000,
  );

  it("renders horizontal and vertical Divider cleanly and stretches vertical Divider inside Toolbar", async () => {
    const horizontal = await catalogStory(
      "Divider",
      "Light0",
      "normal",
      "100%",
    );
    const sepH =
      horizontal.slot.querySelector<HTMLElement>('[role="separator"]');
    if (sepH === null) throw new Error("horizontal divider missing");
    expect(Math.round(sepH.getBoundingClientRect().height)).toBe(1);
    expect(sepH.getBoundingClientRect().width).toBeGreaterThan(100);
    expect(inspectOverflow(horizontal.doc)).toEqual([]);

    const toolbar = await catalogStory("Toolbar", "Light0", "normal", "100%");
    const sepV = toolbar.slot.querySelector<HTMLElement>(
      '[role="separator"][aria-orientation="vertical"]',
    );
    if (sepV === null) throw new Error("vertical divider in toolbar missing");
    expect(Math.round(sepV.getBoundingClientRect().width)).toBe(1);
    expect(sepV.getBoundingClientRect().height).toBeGreaterThanOrEqual(28);
    expect(inspectOverflow(toolbar.doc)).toEqual([]);
  }, 20000);

  it.each(["Light0", "Light1", "Light2"])(
    "keeps Page (%s) free of overflow across normal and narrow slots at 100%% and 125%%",
    async (story) => {
      for (const width of ["normal", "narrow"] as const) {
        for (const scale of ["100%", "125%"] as const) {
          const { doc } = await catalogStory("Page", story, width, scale);
          expect(inspectOverflow(doc)).toEqual([]);
        }
      }
    },
    20000,
  );
});
