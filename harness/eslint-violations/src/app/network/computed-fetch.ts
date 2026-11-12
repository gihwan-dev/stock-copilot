export const load = (
  target: Readonly<Record<"fetch", () => unknown>>,
): unknown => target["fetch"]();
