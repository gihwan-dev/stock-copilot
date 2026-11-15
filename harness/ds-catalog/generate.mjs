import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { desktop, loadCatalog } from "./load.mjs";
const { samples } = await loadCatalog();
const groups = {
  레이아웃: ["Card"],
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
  차트: ["LineChart", "AreaChart", "DonutChart", "Treemap"],
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
    writeFileSync(
      join(desktop, ".storybook/catalog", `${component}.stories.mjs`),
      source,
    );
  }
