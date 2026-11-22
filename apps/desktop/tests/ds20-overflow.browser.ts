import { inspectOverflow } from "../src/shared/ds/testing/overflow";
import { stressFixtures } from "../src/shared/ds/testing/stress";

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
