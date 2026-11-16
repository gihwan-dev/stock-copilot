import { readFileSync } from "node:fs";
import { join } from "node:path";
import { createRequire } from "node:module";
import { desktop } from "./load.mjs";
const { parse } = createRequire(join(desktop, "package.json"))("postcss");
export const stylesheetProblems = (directory) => {
  const html = readFileSync(join(directory, "iframe.html"), "utf8");
  const backgrounds = new Map();
  for (const match of html.matchAll(
    /<link\b[^>]*href=["']([^"']+\.css(?:\?[^"']*)?)["'][^>]*>/g,
  )) {
    const path = match[1].split(/[?#]/)[0].replace(/^\.\//, "");
    if (
      path.startsWith("/") ||
      path.split("/").includes("..") ||
      /^[a-z]+:/i.test(path)
    )
      continue;
    const css = parse(readFileSync(join(directory, path), "utf8"));
    css.walkRules((rule) => {
      if (!rule.selector.includes(":root")) return;
      const theme = /:root\[data-theme=["']?dark/.test(rule.selector)
        ? "dark"
        : !rule.selector.includes("data-theme")
          ? "light"
          : undefined;
      if (theme === undefined) return;
      rule.walkDecls("--color-bg", (declaration) =>
        backgrounds.set(theme, declaration.value),
      );
    });
  }
  return backgrounds.has("light") &&
    backgrounds.has("dark") &&
    backgrounds.get("light") !== backgrounds.get("dark")
    ? []
    : ["목록 테마 CSS가 iframe에 연결되지 않았어요"];
};
