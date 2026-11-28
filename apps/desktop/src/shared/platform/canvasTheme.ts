import {
  useEffect,
  useEffectEvent,
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

const readTokenValues = (target: Readonly<Element>): readonly string[] => {
  const computed = getComputedStyle(target);
  return TOKEN_NAMES.map((token) => computed.getPropertyValue(token).trim());
};

const assemblePalette = (
  tokens: readonly string[],
): CandleThemePalette | undefined => {
  const values = tokens.map((token) => {
    const parsed = parseCanvasColor(token);
    return parsed.kind === "ok" ? parsed.value : undefined;
  });
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
  return assemblePalette(readTokenValues(target));
};

export const readCanvasTheme = (): CandleThemePalette | undefined =>
  readPaletteFromElement(document.documentElement);

const readRootTokenKey = (): string => {
  if (typeof getComputedStyle !== "function") return "";
  return readTokenValues(document.documentElement).join("\u001f");
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

const readColorScheme = (): "light" | "dark" => {
  const forced = document.documentElement.getAttribute("data-theme");
  if (forced === "dark" || forced === "light") return forced;
  return typeof matchMedia === "function" &&
    matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
};

export const useColorScheme = (): "light" | "dark" =>
  useSyncExternalStore(
    subscribeRootTheme,
    readColorScheme,
    (): "light" | "dark" => "light",
  );

export const useCanvasChartTheme = (
  theme: "light" | "dark",
): CandleThemePalette | undefined => {
  const raw = useSyncExternalStore(
    subscribeRootTheme,
    (): string => `${theme}\u001f${readRootTokenKey()}`,
    (): string => "",
  );
  return useMemo(() => assemblePalette(raw.split("\u001f").slice(1)), [raw]);
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
  const { width, height } = canvas;
  if (width === 0 || height === 0) return;
  const density = devicePixelRatio;
  const ctx = canvas.getContext("2d");
  if (ctx === null) return;
  const hairline = Math.max(1, Math.round(density));
  ctx.clearRect(0, 0, width, height);
  ctx.fillStyle = palette.surface;
  ctx.fillRect(0, 0, width, height);
  ctx.strokeStyle = palette.grid;
  ctx.lineWidth = hairline;
  [0.2, 0.5, 0.8].forEach((ratio): void => {
    const y = Math.floor(height * ratio) + (hairline % 2) / 2;
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(width, y);
    ctx.stroke();
  });
  scene.bars.forEach((bar): void => {
    const tone = barTone(palette, bar.direction);
    const cx = Math.round((bar.x / 100) * width);
    const slotWidth = (bar.span / 100) * width;
    const maxWidth = Math.min(16 * density, Math.floor(slotWidth) - 1);
    const barWidth = Math.max(
      1,
      Math.round(Math.min(maxWidth, slotWidth * 0.6)),
    );
    const left = cx - Math.floor(barWidth / 2);
    const wickWidth = Math.min(hairline, barWidth);
    const wickX = left + Math.floor((barWidth - wickWidth) / 2) + wickWidth / 2;
    const wickTop = (bar.wickTop / 60) * height;
    const wickBottom = (bar.wickBottom / 60) * height;
    const bodyTop = (bar.bodyTop / 60) * height;
    const bodyHeight = Math.max(
      hairline,
      ((bar.bodyBottom - bar.bodyTop) / 60) * height,
    );
    ctx.fillStyle = tone;
    ctx.strokeStyle = tone;
    ctx.lineWidth = wickWidth;
    ctx.beginPath();
    ctx.moveTo(wickX, wickTop);
    ctx.lineTo(wickX, wickBottom);
    ctx.stroke();
    ctx.fillRect(left, bodyTop, barWidth, bodyHeight);
    if (scene.activeId === bar.id) {
      ctx.strokeStyle = palette.focus;
      ctx.beginPath();
      ctx.moveTo(wickX, 0);
      ctx.lineTo(wickX, height);
      ctx.stroke();
    }
  });
};

export const useCandleCanvas = (
  canvasRef: Readonly<RefObject<HTMLCanvasElement | null>>,
  scene: CandleCanvasScene,
): void => {
  const draw = useEffectEvent((): void => {
    const canvas = canvasRef.current;
    if (canvas === null) return;
    const box = canvas.getBoundingClientRect();
    const width = Math.round(box.width * devicePixelRatio);
    const height = Math.round(box.height * devicePixelRatio);
    if (canvas.width !== width) canvas.width = width;
    if (canvas.height !== height) canvas.height = height;
    const resolved =
      scene.palette ??
      readPaletteFromElement(canvas.closest("[data-chart-theme]") ?? canvas);
    if (resolved !== undefined) paintScene(canvas, scene, resolved);
  });
  useEffect(() => {
    draw();
  }, [scene, canvasRef]);
  useEffect((): (() => void) | undefined => {
    const canvas = canvasRef.current;
    if (canvas === null) return undefined;
    const scope = canvas.closest("[data-chart-theme]") ?? canvas;
    const densityChanges = new AbortController();
    const watchDensity = (): void => {
      if (typeof matchMedia !== "function") return;
      matchMedia(
        `(resolution: ${devicePixelRatio.toString()}dppx)`,
      ).addEventListener(
        "change",
        () => {
          draw();
          watchDensity();
        },
        { once: true, signal: densityChanges.signal },
      );
    };
    watchDensity();
    const unsubscribe = subscribeThemeChanges(scope, draw);
    const resize =
      typeof ResizeObserver === "function"
        ? new ResizeObserver(draw)
        : undefined;
    resize?.observe(canvas);
    return (): void => {
      densityChanges.abort();
      unsubscribe();
      resize?.disconnect();
    };
  }, [canvasRef]);
};
