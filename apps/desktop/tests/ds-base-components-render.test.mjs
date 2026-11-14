import assert from "node:assert/strict";
import test from "node:test";
import {
  mkdtempSync,
  writeFileSync,
  readFileSync,
  readdirSync,
  rmSync,
} from "node:fs";
import { join } from "node:path";
import { build } from "vite";
import { render } from "@testing-library/react";
import { chromium } from "playwright";
import { desktopRoot, repoRoot } from "./paths.mjs";

const design = readFileSync(
  join(repoRoot, "design/components/bundle.js"),
  "utf8",
);
const paths = JSON.parse(design.match(/var ICONS = (\{.*?\});/u)[1]);

for (const theme of ["light", "dark"]) {
  test(`DS03 Icon: all design glyphs, sizes, names and decorative states in ${theme}`, async () => {
    const folder = mkdtempSync(join(desktopRoot, "node_modules/.ds03-render-"));
    let browser;
    try {
      const entry = join(folder, "entry.mjs");
      writeFileSync(
        entry,
        `
import { createElement } from "react";
import { render, cleanup } from "@testing-library/react";
import { Icon, iconStories } from ${JSON.stringify(join(desktopRoot, "src/shared/ds/index.ts"))};
import ${JSON.stringify(join(desktopRoot, "src/shared/ds/theme.css"))};
window.runIconStories = (theme) => {
cleanup();
document.documentElement.dataset.theme=theme;
const view=render(createElement("div",null,...iconStories.map((story,index)=>createElement("div",{"data-testid":"icon-"+index,key:index},story.render())),createElement(Icon,{name:"search"})));
return {renderer:render.name,count:iconStories.length,defaultWidth:getComputedStyle(view.container.lastElementChild.lastElementChild).width,items:iconStories.map((story,index)=>{
const svg=view.getByTestId("icon-"+index).querySelector("svg");
const style=getComputedStyle(svg);
return {name:story.name,size:story.size,appearance:story.appearance,themes:story.themes,role:svg.getAttribute("role"),label:svg.getAttribute("aria-label"),hidden:svg.getAttribute("aria-hidden"),focusable:svg.getAttribute("focusable"),viewBox:svg.getAttribute("viewBox"),width:style.width,height:style.height,strokeWidth:style.strokeWidth,stroke:style.stroke,color:style.color,fill:style.fill,cap:style.strokeLinecap,join:style.strokeLinejoin,geometry:Array.from(svg.children).map(node=>[node.tagName,Array.from(node.attributes).map(a=>[a.name,a.value]).sort()])};
})};
};`,
      );
      const dist = join(folder, "dist");
      await build({
        root: desktopRoot,
        configFile: join(desktopRoot, "vite.config.mjs"),
        logLevel: "error",
        define: { "process.env.NODE_ENV": '"development"' },
        build: {
          lib: { entry, name: "Ds03", formats: ["iife"] },
          outDir: dist,
          emptyOutDir: true,
          minify: false,
        },
      });
      const files = readdirSync(dist);
      const script = readFileSync(
        join(
          dist,
          files.find((f) => f.endsWith(".js")),
        ),
        "utf8",
      );
      const css = readFileSync(
        join(
          dist,
          files.find((f) => f.endsWith(".css")),
        ),
        "utf8",
      );
      browser = await chromium.launch({ headless: true });
      const page = await browser.newPage();
      const errors = [];
      page.on("pageerror", (error) => errors.push(error.message));
      await page.setContent(
        "<!doctype html><html><head></head><body></body></html>",
      );
      await page.addStyleTag({ content: css });
      await page.addScriptTag({ content: script });
      const result = await page.evaluate(
        "window.runIconStories(" + JSON.stringify(theme) + ")",
      );
      assert.equal(result.renderer, render.name);
      assert.equal(result.count, 270);
      assert.equal(result.defaultWidth, "16px");
      assert.deepEqual(
        [...new Set(result.items.map((item) => item.name))].sort(),
        Object.keys(paths).sort(),
      );
      const expected = await page.evaluate(
        "(" +
          '(paths) =>\n          Object.fromEntries(\n            Object.entries(paths).map(([name, body]) => {\n              const root = new DOMParser().parseFromString(\n                \'<svg xmlns="http://www.w3.org/2000/svg">\' + body + "</svg>",\n                "image/svg+xml",\n              ).documentElement;\n              return [\n                name,\n                Array.from(root.children).map((node) => [\n                  node.tagName,\n                  Array.from(node.attributes)\n                    .map((a) => [a.name, a.value])\n                    .sort(),\n                ]),\n              ];\n            }),\n          )' +
          ")(" +
          JSON.stringify(paths) +
          ")",
      );
      for (const item of result.items) {
        assert.deepEqual(item.geometry, expected[item.name], item.name);
        assert.deepEqual(item.themes, ["light", "dark"]);
        assert.equal(item.viewBox, "0 0 24 24");
        assert.equal(item.focusable, "false");
        assert.equal(
          item.width,
          { "icon-sm": "16px", "icon-md": "20px", "icon-lg": "24px" }[
            item.size
          ],
        );
        assert.equal(item.height, item.width);
        assert.equal(item.strokeWidth, "2px");
        assert.equal(item.stroke, item.color);
        assert.equal(item.fill, "none");
        assert.equal(item.cap, "round");
        assert.equal(item.join, "round");
        if (item.appearance === "meaningful") {
          assert.equal(item.role, "img");
          assert.equal(item.label, "주식 코파일럿");
          assert.equal(item.hidden, null);
        } else {
          assert.equal(item.role, null);
          assert.equal(item.label, null);
          assert.equal(item.hidden, "true");
        }
      }
      await page.keyboard.press("Tab");
      assert.notEqual(
        await page.evaluate("document.activeElement.tagName"),
        "svg",
      );
      assert.deepEqual(errors, []);
    } finally {
      await browser?.close();
      rmSync(folder, { recursive: true, force: true });
    }
  });
}
