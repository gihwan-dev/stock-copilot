import "../src/shared/ds/theme.css";
export default {
  parameters: {
    controls: { disable: true },
    options: { storySort: { method: "alphabetical" } },
  },
  decorators: [
    (Story, context) => {
      document.documentElement.dataset.theme = context.parameters.theme;
      return Story();
    },
  ],
};
