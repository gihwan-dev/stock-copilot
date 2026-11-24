import "../src/shared/ds/theme.css";
export default {
  parameters: {
    controls: { disable: true },
    options: {
      storySort: {
        method: "alphabetical",
        order: [
          "레이아웃",
          [
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
          "기본",
          [
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
          "피드백",
          ["Table", "Badge", "StatusBar", "Dialog", "EmptyState", "Skeleton"],
          "숫자",
          [
            "Money",
            "Percent",
            "Quantity",
            "Probability",
            "PriceChange",
            "DirectionBadge",
          ],
          "차트",
          ["LineChart", "AreaChart", "DonutChart", "Treemap"],
          "콘텐츠",
          ["Markdown", "ChatInput"],
        ],
      },
    },
  },
  decorators: [
    (Story, context) => {
      const theme = context.parameters.theme;
      if (theme !== "light" && theme !== "dark")
        throw new Error("이야기의 테마는 light 또는 dark여야 해요");
      document.documentElement.dataset.theme = theme;
      document.documentElement.classList.add(
        "bg-surface",
        "text-ink",
        "h-full",
      );
      document.body.classList.add("h-full");
      document.getElementById("storybook-root")?.classList.add("h-full");
      return Story();
    },
  ],
};
