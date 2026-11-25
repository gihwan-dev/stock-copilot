import { settings } from "./policy.mjs";
export const captureStory = async (
  browser,
  { origin, entry, component, theme },
) => {
  const context = await browser.newContext({
    viewport: settings.viewport,
    deviceScaleFactor: settings.scale,
    locale: settings.locale,
    timezoneId: settings.timezone,
    colorScheme: theme,
    reducedMotion: "reduce",
    serviceWorkers: "block",
  });

  try {
    await context.route("**/*", (route) =>
      new URL(route.request().url()).origin === origin
        ? route.continue()
        : route.abort(),
    );
    await context.addInitScript(
      ({ random }) => {
        Math.random = () => random;
      },
      { random: settings.random },
    );
    const page = await context.newPage();
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.clock.install({ time: new Date(0) });
    await page.clock.pauseAt(new Date(settings.time));
    await page.goto(
      `${origin}/iframe.html?id=${encodeURIComponent(entry.id)}&viewMode=story`,
      { waitUntil: "networkidle" },
    );
    await page
      .locator(
        `[data-catalog-component="${component}"][data-catalog-theme="${theme}"]`,
      )
      .waitFor();
    await page.addStyleTag({
      content: `*, *::before, *::after { animation: none !important; transition: none !important; caret-color: transparent !important; }`,
    });
    const fonts = await page.evaluate(async () => {
      await globalThis.document.fonts.ready;
      await Promise.all([
        globalThis.document.fonts.load('400 16px "IBM Plex Sans KR"'),
        globalThis.document.fonts.load('400 16px "IBM Plex Mono"'),
      ]);
      return [...globalThis.document.fonts]
        .filter((font) => font.status === "loaded")
        .map((font) => font.family);
    });
    if (
      !fonts.includes("IBM Plex Sans KR") ||
      !fonts.includes("IBM Plex Mono") ||
      errors.length !== 0
    )
      throw new Error(`글꼴/페이지 실패: ${JSON.stringify({ fonts, errors })}`);
    const inputs = await page.evaluate(() => ({
      time: Date.now(),
      random: Math.random(),
    }));
    if (
      inputs.time !== Date.parse(settings.time) ||
      inputs.random !== settings.random
    )
      throw new Error("시계·난수 고정 실패");
    const session = await context.newCDPSession(page);
    await session.send("DOM.enable");
    await session.send("CSS.enable");
    const { root: dom } = await session.send("DOM.getDocument");
    const { nodeIds } = await session.send("DOM.querySelectorAll", {
      nodeId: dom.nodeId,
      selector: "[data-catalog-component], [data-catalog-component] *",
    });
    for (const nodeId of nodeIds) {
      const { fonts: used } = await session.send(
        "CSS.getPlatformFontsForNode",
        { nodeId },
      );
      if (used.some((font) => font.glyphCount > 0 && !font.isCustomFont))
        throw new Error(`시스템 글꼴 사용: ${JSON.stringify(used)}`);
    }
    await session.detach();
    await page.clock.runFor(settings.settle);
    const bytes = await page.screenshot({
      animations: settings.animations,
      caret: settings.caret,
    });
    return bytes;
  } finally {
    await context.close();
  }
};
