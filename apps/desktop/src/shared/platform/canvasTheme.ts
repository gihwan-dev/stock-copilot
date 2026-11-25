import {
  useCallback,
  useEffect,
  useMemo,
  useSyncExternalStore,
  type RefObject,
} from "react";
import {
  parseCanvasColor,
  type CandleCanvasScene,
  type CandleThemePalette,
  type CanvasCandleMark,
  type CanvasColor,
} from "../lib";

const TOKEN_NAMES = [
  "--color-surface",
  "--color-line",
  "--color-ink-muted",
  "--color-up",
  "--color-down",
  "--color-focus-ring",
] as const;

const readTokenValues = (
  computed: Readonly<CSSStyleDeclaration>,
): readonly (CanvasColor | undefined)[] =>
  TOKEN_NAMES.map((token) => {
    const parsed = parseCanvasColor(computed.getPropertyValue(token));
    return parsed.kind === "ok" ? parsed.value : undefined;
  });

const assemblePalette = (
  values: readonly (CanvasColor | undefined)[],
): CandleThemePalette | undefined => {
  const [surface, grid, axisText, rising, falling, focus] = values;
  if (
    surface === undefined ||
    grid === undefined ||
    axisText === undefined ||
    rising === undefined ||
    falling === undefined ||
    focus === undefined
  )
    return undefined;
  return { surface, grid, axisText, rising, falling, flat: axisText, focus };
};

const readPaletteFromElement = (
  target: Readonly<Element> | null,
): CandleThemePalette | undefined => {
  if (target === null || typeof getComputedStyle !== "function")
    return undefined;
  return assemblePalette(readTokenValues(getComputedStyle(target)));
};

export const readCanvasTheme = (): CandleThemePalette | undefined =>
  readPaletteFromElement(document.documentElement);

const readRootTokenKey = (): string => {
  if (typeof getComputedStyle !== "function") return "";
  const computed = getComputedStyle(document.documentElement);
  return TOKEN_NAMES.map((token) =>
    computed.getPropertyValue(token).trim(),
  ).join("\u001f");
};

const subscribeThemeChanges = (
  target: Readonly<Element>,
  onChange: () => void,
): (() => void) => {
  const observer =
    typeof MutationObserver === "function"
      ? new MutationObserver(onChange)
      : undefined;
  observer?.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["data-theme", "data-chart-theme"],
  });
  if (target !== document.documentElement) {
    observer?.observe(target, {
      attributes: true,
      attributeFilter: ["data-chart-theme"],
    });
  }
  const media =
    typeof matchMedia === "function"
      ? matchMedia("(prefers-color-scheme: dark)")
      : undefined;
  media?.addEventListener("change", onChange);
  return (): void => {
    observer?.disconnect();
    media?.removeEventListener("change", onChange);
  };
};

const subscribeRootTheme = (onChange: () => void): (() => void) =>
  subscribeThemeChanges(document.documentElement, onChange);

const parseTokenKey = (key: string): CandleThemePalette | undefined => {
  if (key.length === 0) return undefined;
  return assemblePalette(
    key.split("\u001f").map((value) => {
      const parsed = parseCanvasColor(value);
      return parsed.kind === "ok" ? parsed.value : undefined;
    }),
  );
};

export const useCanvasChartTheme = (
  theme: "light" | "dark",
): CandleThemePalette | undefined => {
  const snapshot = useCallback(
    (): string => `${theme}\u001e${readRootTokenKey()}`,
    [theme],
  );
  const raw = useSyncExternalStore(
    subscribeRootTheme,
    snapshot,
    (): string => "",
  );
  return useMemo(() => {
    const separator = raw.indexOf("\u001e");
    return separator < 0 ? undefined : parseTokenKey(raw.slice(separator + 1));
  }, [raw]);
};

const barTone = (
  palette: CandleThemePalette,
  direction: CanvasCandleMark["direction"],
): CanvasColor => {
  switch (direction) {
    case "up":
      return palette.rising;
    case "down":
      return palette.falling;
    case "flat":
      return palette.flat;
  }
};

const paintScene = (
  canvas: Readonly<HTMLCanvasElement>,
  scene: CandleCanvasScene,
  palette: CandleThemePalette,
): void => {
  const ctx = canvas.getContext("2d");
  if (ctx === null) return;
  const width = canvas.width > 0 ? canvas.width : 300;
  const height = canvas.height > 0 ? canvas.height : 150;
  ctx.clearRect(0, 0, width, height);
  ctx.fillStyle = palette.surface;
  ctx.fillRect(0, 0, width, height);
  ctx.strokeStyle = palette.grid;
  ctx.lineWidth = 1;
  [0.2, 0.5, 0.8].forEach((ratio): void => {
    const y = Math.round(height * ratio) + 0.5;
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(width, y);
    ctx.stroke();
  });
  scene.bars.forEach((bar): void => {
    const tone = barTone(palette, bar.direction);
    const cx = (bar.x / 100) * width;
    const barWidth = Math.max(2, (bar.span / 100) * width);
    const wickTop = (bar.wickTop / 60) * height;
    const wickBottom = (bar.wickBottom / 60) * height;
    const bodyTop = (bar.bodyTop / 60) * height;
    const bodyHeight = Math.max(
      1.5,
      ((bar.bodyBottom - bar.bodyTop) / 60) * height,
    );
    ctx.fillStyle = tone;
    ctx.strokeStyle = tone;
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    ctx.moveTo(cx, wickTop);
    ctx.lineTo(cx, wickBottom);
    ctx.stroke();
    ctx.fillRect(cx - barWidth / 2, bodyTop, barWidth, bodyHeight);
    if (scene.activeId === bar.id) {
      ctx.strokeStyle = palette.focus;
      ctx.beginPath();
      ctx.moveTo(cx, 0);
      ctx.lineTo(cx, height);
      ctx.stroke();
    }
  });
};

export const useCandleCanvas = (
  canvasRef: Readonly<RefObject<HTMLCanvasElement | null>>,
  scene: CandleCanvasScene,
): void => {
  useEffect((): (() => void) | undefined => {
    const canvas = canvasRef.current;
    if (canvas === null) return undefined;
    const scope = canvas.closest("[data-chart-theme]") ?? canvas;
    const draw = (): void => {
      const resolved = scene.palette ?? readPaletteFromElement(scope);
      if (resolved !== undefined) paintScene(canvas, scene, resolved);
    };
    draw();
    const unsubscribe = subscribeThemeChanges(scope, draw);
    const resize =
      typeof ResizeObserver === "function"
        ? new ResizeObserver(draw)
        : undefined;
    resize?.observe(canvas);
    return (): void => {
      unsubscribe();
      resize?.disconnect();
    };
  }, [canvasRef, scene]);
};
