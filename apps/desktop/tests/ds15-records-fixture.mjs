import { createHash } from "node:crypto";
import { settings } from "../../../harness/visual/policy.mjs";
export const baselineFixture = () => {
  const entries = settings.components.flatMap((component) =>
    settings.themes.map((theme) => [component, theme]),
  );
  const images = {};
  const measurements = entries.map(([component, theme], index) => {
    const bytes = Buffer.from(`${component}/${theme}/${"x".repeat(index + 1)}`);
    images[`${component}-${theme}-chromium-linux-arm64.png`] = bytes;
    const hash = createHash("sha256").update(bytes).digest("hex");
    return { component, theme, bytes: bytes.length, first: hash, second: hash };
  });
  return { images, measurements };
};
