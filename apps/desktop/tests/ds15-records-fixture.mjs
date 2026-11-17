import { createHash } from "node:crypto";
export const baselineFixture = () => {
  const entries = [
    ["Button", "light"],
    ["Button", "dark"],
    ["Card", "light"],
    ["Card", "dark"],
    ["Money", "light"],
    ["Money", "dark"],
    ["TextField", "light"],
    ["TextField", "dark"],
  ];
  const images = {};
  const measurements = entries.map(([component, theme], index) => {
    const bytes = Buffer.from(`${component}/${theme}/${"x".repeat(index + 1)}`);
    images[`${component}-${theme}-chromium-linux-arm64.png`] = bytes;
    const hash = createHash("sha256").update(bytes).digest("hex");
    return { component, theme, bytes: bytes.length, first: hash, second: hash };
  });
  return { images, measurements };
};
