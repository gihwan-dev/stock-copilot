import { inspectOverflow } from "../../apps/desktop/src/shared/ds/testing/overflow";
import { commands } from "vitest/browser";

beforeAll(async () => {
  await commands.prepareCatalog();
});

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
        frame.contentWindow?.Date.now() !==
          Date.parse("2026-01-02T03:04:05.000Z") ||
        frame.contentWindow?.Math.random() !== 0.3141592653589793
      )
        throw new Error("이야기의 시계·난수가 고정되지 않았다");
      const style = doc.createElement("style");
      style.textContent =
        "*,*::before,*::after{animation:none!important;transition:none!important;caret-color:transparent!important}";
      doc.head.append(style);
      await new Promise((resolve) =>
        frame.contentWindow?.requestAnimationFrame(() =>
          frame.contentWindow?.requestAnimationFrame(resolve),
        ),
      );
      return doc;
    }
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  throw new Error("스토리 로딩 실패");
};

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
  expect(frame.contentWindow?.Date.now()).toBe(
    Date.parse("2026-01-02T03:04:05.000Z"),
  );
  expect(frame.contentWindow?.Math.random()).toBe(0.3141592653589793);
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
      for (const width of ["normal", "narrow"] as const) {
        content.style.boxSizing = width === "narrow" ? "content-box" : "";
        content.style.width = width === "narrow" ? "142px" : "";
        for (const scale of ["100%", "125%"] as const) {
          doc.documentElement.style.fontSize = scale;
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
