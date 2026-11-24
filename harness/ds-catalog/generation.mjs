import { format } from "prettier";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { desktop } from "./load.mjs";
export const groups = {
  레이아웃: [
    "AppShell",
    "Page",
    "Section",
    "Grid",
    "GridItem",
    "Stack",
    "Inline",
    "Split",
    "Card",
    "Toolbar",
    "Divider",
  ],
  기본: [
    "Text",
    "Heading",
    "Button",
    "IconButton",
    "TextField",
    "Select",
    "Checkbox",
    "Switch",
    "SegmentedControl",
    "Disclosure",
    "Icon",
  ],
  피드백: ["Table", "Badge", "StatusBar", "Dialog", "EmptyState", "Skeleton"],
  숫자: [
    "Money",
    "Percent",
    "Quantity",
    "Probability",
    "PriceChange",
    "DirectionBadge",
  ],
  차트: ["LineChart", "AreaChart", "DonutChart", "Treemap", "CandleChart"],
  콘텐츠: ["Markdown", "ChatInput"],
};
const states = {
  normal: "기본",
  disabled: "비활성",
  selected: "선택",
  error: "오류",
  hint: "도움말",
  secret: "가림",
  empty: "빈 상태",
  open: "열림",
  closed: "닫힘",
  decorative: "장식용",
  meaningful: "의미 있음",
  present: "내용 있음",
  absent: "내용 없음",
  single: "한 점",
  load: "부하",
  invalid: "잘못된 자료",
  denied: "링크 제한",
  sending: "전송 중",
  stress: "부하",
  long: "긴 내용",
  representative: "가상 자료",
};
export const generatedCatalogFiles = async (samples) => {
  const files = {};
  files["catalog.mjs"] = await format(
    'import { createElement } from "react";\nimport {\n  baseComponentStories,\n  feedbackComponentStories,\n  statusComponentStories,\n  chartComponentStories,\n  numberComponentStories,\n  contentComponentStories,\n  layoutComponentStories,\n  catalogAdditionalStories,\n} from "../src/shared/ds/stories.ts";\nimport { stressComponentStories } from "../src/shared/ds/testing/Stress.stories.tsx";\nimport { ScrollArea } from "../src/shared/ds/ScrollArea.tsx";\nimport { catalogSamples } from "../../../harness/ds-catalog/samples.mjs";\nconst samples = catalogSamples({\n  baseComponentStories,\n  feedbackComponentStories,\n  statusComponentStories,\n  chartComponentStories,\n  numberComponentStories,\n  contentComponentStories,\n  layoutComponentStories,\n  catalogAdditionalStories,\n  stressComponentStories,\n});\nconst numericComponents = new Set([\n  "Money",\n  "Percent",\n  "Quantity",\n  "Probability",\n  "PriceChange",\n]);\nconst Example = ({ render, theme }) => render(theme);\nconst Stage = ({ component, render, theme }) =>\n  component === "AppShell"\n    ? createElement(\n        "div",\n        {\n          "data-catalog-content": "",\n          className: "flex-1 scroll-flex flex flex-col w-full h-full",\n        },\n        createElement(Example, { render, theme }),\n      )\n    : createElement(\n        ScrollArea,\n        { direction: "vertical" },\n        numericComponents.has(component)\n          ? createElement(\n              "div",\n              { className: "py-space-3" },\n              createElement(\n                ScrollArea,\n                { direction: "horizontal" },\n                createElement(\n                  "div",\n                  { "data-catalog-content": "", className: "w-max" },\n                  createElement(Example, { render, theme }),\n                ),\n              ),\n            )\n          : createElement(\n              "div",\n              {\n                "data-catalog-content": "",\n                className: "w-full scroll-flex py-space-3",\n              },\n              createElement(Example, { render, theme }),\n            ),\n      );\nexport const renderCatalog = (component, theme, sample) => {\n  const story = samples.find(\n    (story) => story.component === component && story.sample === sample,\n  );\n  if (story === undefined || (theme !== "light" && theme !== "dark"))\n    throw new Error("목록의 컴포넌트·예제·테마가 올바르지 않아요");\n  return createElement(\n    "div",\n    {\n      className:\n        component === "AppShell"\n          ? "h-full flex flex-col bg-surface text-ink"\n          : "h-full flex flex-col bg-surface text-ink p-space-4",\n      "data-catalog-component": component,\n      "data-catalog-theme": theme,\n      "data-catalog-sample": sample,\n    },\n    createElement(\n      "section",\n      {\n        key: `${component}-${sample}-${theme}`,\n        className:\n          component === "AppShell"\n            ? "flex-1 scroll-flex flex flex-col bg-surface"\n            : "flex-1 scroll-flex flex flex-col bg-surface px-space-4",\n      },\n      createElement(Stage, { component, render: story.render, theme }),\n    ),\n  );\n};\n',
    { filepath: "catalog.mjs" },
  );
  for (const [group, components] of Object.entries(groups))
    for (const component of components) {
      const stories = samples.filter((story) => story.component === component);
      let source = `import { renderCatalog } from "../catalog.mjs";\nexport default { title: ${JSON.stringify(`${group}/${component}`)} };\n`;
      for (const story of stories)
        for (const theme of ["light", "dark"]) {
          const element = story.render(theme);
          const details = [
            "name",
            "variant",
            "size",
            "level",
            "tone",
            "notation",
            "direction",
          ]
            .map((key) => element?.props?.[key])
            .filter(
              (value) => typeof value === "string" || typeof value === "number",
            );
          const name = [
            states[story.state] ?? "기본",
            ...details,
            (story.sample + 1).toString().padStart(3, "0"),
            theme === "light" ? "라이트" : "다크",
          ].join(" · ");
          source += `export const ${theme === "light" ? "Light" : "Dark"}${story.sample} = { name: ${JSON.stringify(name)}, parameters: { theme: "${theme}", sample: ${story.sample} }, render: () => renderCatalog("${component}", "${theme}", ${story.sample}) };\n`;
        }
      files[`catalog/${component}.stories.mjs`] = await format(source, {
        filepath: `${component}.mjs`,
      });
    }
  files["main.mjs"] = await format(
    'export default {\n  framework: "@storybook/react-vite",\n  stories: ["./catalog/*.stories.mjs"],\n  core: { disableTelemetry: true },\n};\n',
    { filepath: "main.mjs" },
  );
  files["preview.mjs"] = await format(
    'import "../src/shared/ds/theme.css";\nexport default {\n parameters: {controls: {disable: true}, options: {storySort: {method: "alphabetical", order: ORDER}}},\n decorators: [(Story, context) => {const theme=context.parameters.theme;\n if(theme!=="light" && theme!=="dark") throw new Error("이야기의 테마는 light 또는 dark여야 해요");\n document.documentElement.dataset.theme=theme;document.documentElement.classList.add("bg-surface","text-ink","h-full");document.body.classList.add("h-full");document.getElementById("storybook-root")?.classList.add("h-full");return Story();}],\n};\n'.replace(
      "ORDER",
      JSON.stringify(
        Object.entries(groups).flatMap(([group, components]) => [
          group,
          components,
        ]),
      ),
    ),
    { filepath: "preview.mjs" },
  );
  return files;
};
export const generationProblems = async (samples) => {
  const expected = await generatedCatalogFiles(samples);
  const problems = Object.entries(expected)
    .filter(
      ([file, source]) =>
        readFileSync(join(desktop, ".storybook", file), "utf8") !== source,
    )
    .map(([file]) => `목록 생성물 불일치: ${file}`);
  const files = readdirSync(join(desktop, ".storybook/catalog")).filter(
    (file) => file.endsWith(".stories.mjs"),
  );
  for (const file of files)
    if (expected[`catalog/${file}`] === undefined)
      problems.push(`등록되지 않은 목록 파일: ${file}`);
  for (const file of readdirSync(join(desktop, ".storybook")))
    if (
      (/^(?:main|preview|manager)\.(?:[cm]?js|jsx|tsx?|mts|cts)$/.test(file) &&
        !["main.mjs", "preview.mjs"].includes(file)) ||
      /^(?:preview|manager)-(?:head|body)\.html$/.test(file)
    )
      problems.push(`지원하지 않는 목록 설정: ${file}`);
  return problems;
};
