import { inspectOverflow } from "../src/shared/ds/testing/overflow";
import { stressFixtures } from "../src/shared/ds/testing/stress";
import { commands } from "vitest/browser";

declare const __DS20_FIRST__: number;
declare const __DS20_SIZE__: number;

beforeAll(async () => {
  await commands.prepareCatalog();
});

const waitForLayout = async (frame: HTMLIFrameElement): Promise<void> => {
  const view = frame.contentWindow;
  if (view === null) throw new Error("이야기 창 누락");
  await new Promise<void>((resolve) =>
    view.requestAnimationFrame(() =>
      view.requestAnimationFrame(() => resolve()),
    ),
  );
};

const waitForCatalog = async (frame: HTMLIFrameElement): Promise<Document> => {
  const deadline = Date.now() + 10000;
  while (Date.now() < deadline) {
    const doc = frame.contentDocument;
    if (
      doc?.querySelector("[data-catalog-component]") !== null &&
      doc !== null
    ) {
      await doc.fonts.ready;
      const fonts = await Promise.all([
        doc.fonts.load('400 16px "IBM Plex Sans KR"'),
        doc.fonts.load('400 16px "IBM Plex Mono"'),
      ]);
      if (fonts.some((items) => items.length === 0))
        throw new Error("묶은 글꼴 로딩 실패");
      if (
        (
          frame.contentWindow as (Window & typeof globalThis) | null
        )?.Date.now() !== Date.parse("2026-01-02T03:04:05.000Z") ||
        (
          frame.contentWindow as (Window & typeof globalThis) | null
        )?.Math.random() !== 0.3141592653589793
      )
        throw new Error("이야기의 시계·난수가 고정되지 않았다");
      const style = doc.createElement("style");
      style.textContent =
        "*,*::before,*::after{animation:none!important;transition:none!important;caret-color:transparent!important}";
      doc.head.append(style);
      await waitForLayout(frame);
      return doc;
    }
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  throw new Error("스토리 로딩 실패");
};

const hasArea = (element: Readonly<Element>): boolean =>
  [...element.getClientRects()].some(
    (rect) => rect.width > 0 && rect.height > 0,
  );

const hasStressContent = (
  doc: Readonly<Document>,
  content: Readonly<Element>,
): boolean => {
  const range = doc.createRange();
  range.selectNodeContents(content);
  const textVisible =
    (content.textContent ?? "").trim() !== "" &&
    [...range.getClientRects()].some(
      (rect) => rect.width > 0 && rect.height > 0,
    );
  const portal = doc.querySelector('[role="dialog"]');
  return (
    [...content.children].some(hasArea) ||
    textVisible ||
    (doc.querySelector('[data-catalog-component="Dialog"]') !== null &&
      portal !== null &&
      hasArea(portal))
  );
};

it("rejects empty stress content while accepting an open dialog portal", () => {
  const content = document.createElement("section");
  const wrapper = document.createElement("div");
  wrapper.setAttribute("data-catalog-component", "Dialog");
  wrapper.append(content);
  document.body.append(wrapper);
  try {
    expect(hasStressContent(document, content)).toBe(false);
    const dialog = document.createElement("div");
    dialog.setAttribute("role", "dialog");
    dialog.style.width = "20px";
    dialog.style.height = "20px";
    document.body.append(dialog);
    expect(hasStressContent(document, content)).toBe(true);
    dialog.remove();
    const hidden = document.createElement("div");
    hidden.style.display = "none";
    content.append(hidden);
    expect(hasStressContent(document, content)).toBe(false);
    const empty = document.createElement("span");
    empty.style.display = "inline-block";
    empty.style.width = "0";
    empty.style.height = "0";
    content.append(empty);
    expect(hasStressContent(document, content)).toBe(false);
    empty.remove();
    content.append(document.createTextNode("보이는 내용"));
    expect(hasStressContent(document, content)).toBe(true);
  } finally {
    wrapper.remove();
  }
});

it("waits for resize-observer layout updates before measurement", async () => {
  const frame = document.createElement("iframe");
  const loaded = new Promise<void>((resolve) =>
    frame.addEventListener("load", () => resolve(), { once: true }),
  );
  frame.srcdoc =
    '<!doctype html><html><body><div id="container" style="width:120px"><div id="child"></div></div></body></html>';
  document.body.append(frame);
  try {
    await loaded;
    const view = frame.contentWindow as (Window & typeof globalThis) | null;
    const doc = frame.contentDocument;
    const container = doc?.querySelector<HTMLElement>("#container");
    const child = doc?.querySelector<HTMLElement>("#child");
    if (view === null || container == null || child == null)
      throw new Error("크기 관찰 예제 누락");
    const observer = new view.ResizeObserver(() => {
      child.style.width = `${container.clientWidth}px`;
    });
    try {
      observer.observe(container);
      await waitForLayout(frame);
      expect(child.clientWidth).toBe(120);
      container.style.width = "40px";
      expect(child.clientWidth).toBe(120);
      await waitForLayout(frame);
      expect(child.clientWidth).toBe(40);
    } finally {
      observer.disconnect();
    }
  } finally {
    frame.remove();
  }
});

it("measures an actual Storybook component in the fixed browser", async () => {
  const response = await fetch("/index.json");
  const index = await response.json();
  const entries = Object.values(index.entries) as {
    id: string;
    type: string;
    importPath: string;
    exportName: string;
  }[];
  const story = entries.find(
    (entry) =>
      entry.importPath.endsWith("/Button.stories.mjs") &&
      entry.exportName === "Light0",
  );
  expect(story).toBeDefined();
  if (story === undefined) throw new Error("Button 이야기 없음");
  const frame = document.createElement("iframe");
  frame.width = "960";
  frame.height = "640";
  frame.src = `/iframe.html?id=${encodeURIComponent(story.id)}&viewMode=story`;
  document.body.append(frame);
  const doc = await waitForCatalog(frame);
  expect(doc.querySelector("button")).not.toBeNull();
  expect(
    (frame.contentWindow as (Window & typeof globalThis) | null)?.Date.now(),
  ).toBe(Date.parse("2026-01-02T03:04:05.000Z"));
  expect(
    (frame.contentWindow as (Window & typeof globalThis) | null)?.Math.random(),
  ).toBe(0.3141592653589793);
  expect(inspectOverflow(doc)).toEqual([]);
  frame.remove();
}, 20000);

it("does not make the document scroll for a 500-point screen-reader table", async () => {
  const response = await fetch("/index.json");
  const index = await response.json();
  const entries = Object.values(index.entries) as {
    id: string;
    importPath: string;
    exportName: string;
  }[];
  for (const component of ["LineChart", "AreaChart"]) {
    const story = entries.find(
      (entry) =>
        entry.importPath.endsWith(`/${component}.stories.mjs`) &&
        entry.exportName === "Light4",
    );
    expect(story).toBeDefined();
    if (story === undefined) throw new Error(`${component} 부하 이야기 없음`);
    const frame = document.createElement("iframe");
    frame.width = "960";
    frame.height = "640";
    frame.src = `/iframe.html?id=${encodeURIComponent(story.id)}&viewMode=story`;
    document.body.append(frame);
    try {
      const doc = await waitForCatalog(frame);
      expect(doc.querySelectorAll("table.sr-only tr").length).toBe(500);
      expect(
        inspectOverflow(doc).filter(
          (finding) => finding.kind === "document-scroll",
        ),
      ).toEqual([]);
    } finally {
      frame.remove();
    }
  }
}, 20000);

it("measures every catalog story in the narrow slot and at enlarged text", async () => {
  const response = await fetch("/index.json");
  const index = await response.json();
  const entries = Object.values(index.entries) as {
    id: string;
    type: string;
    name: string;
    title: string;
  }[];
  const allStories = entries.filter((entry) => entry.type === "story");
  const stories = allStories.slice(
    __DS20_FIRST__,
    __DS20_FIRST__ + __DS20_SIZE__,
  );
  const results = [];
  for (const story of stories) {
    const frame = document.createElement("iframe");
    frame.width = "960";
    frame.height = "640";
    frame.src = `/iframe.html?id=${encodeURIComponent(story.id)}&viewMode=story`;
    document.body.append(frame);
    try {
      const doc = await waitForCatalog(frame);
      const content = doc.querySelector<HTMLElement>(
        "[data-catalog-component] > section",
      );
      if (content === null) throw new Error(`이야기 본문 누락: ${story.id}`);
      if (story.name.includes("부하") && !hasStressContent(doc, content))
        throw new Error(`부하 이야기 내용 누락: ${story.id}`);
      for (const width of ["normal", "narrow"] as const) {
        content.style.boxSizing = width === "narrow" ? "content-box" : "";
        content.style.width = width === "narrow" ? "126px" : "";
        for (const scale of ["100%", "125%"] as const) {
          doc.documentElement.style.fontSize = scale;
          await waitForLayout(frame);
          const findings = inspectOverflow(doc);
          if (findings.length > 0)
            results.push({
              story: story.title,
              id: story.id,
              width,
              scale,
              findings,
            });
        }
      }
    } finally {
      frame.remove();
    }
  }
  const errors = await commands.catalogErrors();
  console.log(
    "DS20_CATALOG_FINDINGS " +
      JSON.stringify({
        first: __DS20_FIRST__,
        count: stories.length,
        total: allStories.length,
        errors,
        results,
      }),
  );
  expect(stories.length).toBeGreaterThan(0);
  expect(errors).toEqual([]);
  expect(results).toEqual([]);
}, 600000);

it("wraps long English text in the narrow stress slot", async () => {
  const response = await fetch("/index.json");
  const index = await response.json();
  const entries = Object.values(index.entries) as {
    id: string;
    importPath: string;
    exportName: string;
    name: string;
  }[];
  const stories = entries.filter(
    (entry) =>
      entry.importPath.endsWith("/Text.stories.mjs") &&
      entry.exportName.startsWith("Light") &&
      entry.name.includes("부하"),
  );
  const results: { width: number; scroll: number }[] = [];
  for (const story of stories) {
    const frame = document.createElement("iframe");
    frame.width = "960";
    frame.height = "640";
    frame.src = `/iframe.html?id=${encodeURIComponent(story.id)}&viewMode=story`;
    document.body.append(frame);
    try {
      const doc = await waitForCatalog(frame);
      const slot = doc.querySelector<HTMLElement>(
        "[data-catalog-component] > section",
      );
      if (slot === null) throw new Error("이야기 본문 누락");
      slot.style.boxSizing = "content-box";
      slot.style.width = "126px";
      const text = slot.querySelector("p");
      if (text?.textContent?.includes(stressFixtures.english))
        results.push({ width: text.clientWidth, scroll: text.scrollWidth });
    } finally {
      frame.remove();
    }
  }
  expect(results).toHaveLength(1);
  expect(results[0]?.scroll).toBeLessThanOrEqual(results[0]?.width ?? 0);
}, 60000);

it("measures the real ChatInput draft and keeps its nested exception narrow", async () => {
  const response = await fetch("/index.json");
  const index = await response.json();
  const entries = Object.values(index.entries) as {
    id: string;
    importPath: string;
    exportName: string;
    name: string;
  }[];
  const stories = entries.filter(
    (entry) =>
      entry.importPath.endsWith("/ChatInput.stories.mjs") &&
      entry.exportName.startsWith("Light") &&
      entry.name.includes("부하"),
  );
  let matches = 0;
  for (const story of stories) {
    const frame = document.createElement("iframe");
    frame.width = "960";
    frame.height = "640";
    frame.src = `/iframe.html?id=${encodeURIComponent(story.id)}&viewMode=story`;
    document.body.append(frame);
    try {
      const doc = await waitForCatalog(frame);
      const chat = doc.querySelector<HTMLTextAreaElement>(
        "textarea.chat-input",
      );
      if (chat?.value.includes(stressFixtures.company)) {
        matches += 1;
        expect(chat.value).toBe(
          [stressFixtures.company, ...stressFixtures.paragraphs].join("\n"),
        );
        expect(chat.scrollHeight).toBeGreaterThan(chat.clientHeight);
        expect(inspectOverflow(doc)).not.toContainEqual({
          kind: "unauthorized-overflow",
          element: "textarea",
        });
        const overflowLabel = doc.createElement("label");
        overflowLabel.id = "overflow-label";
        overflowLabel.style.width = "20px";
        overflowLabel.style.display = "block";
        overflowLabel.style.whiteSpace = "nowrap";
        overflowLabel.textContent = "labeltextlabeltextlabeltext";
        doc.body.append(overflowLabel);
        expect(inspectOverflow(doc)).toContainEqual({
          kind: "unauthorized-overflow",
          element: "label#overflow-label",
        });
        overflowLabel.remove();
        const parent = chat.parentElement;
        if (parent === null) throw new Error("ChatInput 부모 누락");
        const viewport = doc.createElement("div");
        viewport.setAttribute("data-scroll-viewport", "");
        viewport.style.overflow = "auto";
        viewport.style.height = "100px";
        parent.insertBefore(viewport, chat);
        viewport.append(chat);
        expect(inspectOverflow(doc)).not.toContainEqual({
          kind: "nested-scroll",
          element: "textarea",
        });
      }
    } finally {
      frame.remove();
    }
  }
  expect(matches).toBe(1);
}, 60000);

it("reports native TextField overflow without assigning a viewport marker", async () => {
  const response = await fetch("/index.json");
  const index = await response.json();
  const entries = Object.values(index.entries) as {
    id: string;
    importPath: string;
    exportName: string;
    name: string;
  }[];
  const stories = entries.filter(
    (entry) =>
      entry.importPath.endsWith("/TextField.stories.mjs") &&
      entry.exportName.startsWith("Light") &&
      entry.name.includes("부하"),
  );
  let matches = 0;
  for (const story of stories) {
    const frame = document.createElement("iframe");
    frame.width = "960";
    frame.height = "640";
    frame.src = `/iframe.html?id=${encodeURIComponent(story.id)}&viewMode=story`;
    document.body.append(frame);
    try {
      const doc = await waitForCatalog(frame);
      const slot = doc.querySelector<HTMLElement>(
        "[data-catalog-component] > section",
      );
      const input = doc.querySelector<HTMLInputElement>("input");
      if (slot === null || input === null) throw new Error("TextField 누락");
      if (input.value === stressFixtures.digits) {
        matches += 1;
        slot.style.boxSizing = "content-box";
        slot.style.width = "126px";
        await waitForLayout(frame);
        expect(input.scrollWidth).toBeGreaterThan(input.clientWidth);
        expect(input.hasAttribute("data-scroll-viewport")).toBe(false);
        expect(
          inspectOverflow(doc).some(
            (finding) =>
              finding.kind === "unauthorized-overflow" &&
              finding.element.startsWith("input"),
          ),
        ).toBe(false);
        const overflowHint = doc.createElement("div");
        overflowHint.id = "overflow-hint";
        overflowHint.style.width = "20px";
        overflowHint.style.whiteSpace = "nowrap";
        overflowHint.textContent = "hinttexthinttexthinttext";
        doc.body.append(overflowHint);
        expect(inspectOverflow(doc)).toContainEqual({
          kind: "unauthorized-overflow",
          element: "div#overflow-hint",
        });
        overflowHint.remove();
      }
    } finally {
      frame.remove();
    }
  }
  expect(matches).toBe(1);
}, 60000);
