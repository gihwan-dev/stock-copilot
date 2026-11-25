export const openAdvancedSource = `
  window.openAdvanced = () => {
    let attempts = 0;
    const timer = window.setInterval(() => {
      attempts += 1;
      const trigger = Array.from(document.querySelectorAll("button")).find(
        (node) =>
          node.textContent === "고급" &&
          node.getAttribute("aria-expanded") !== "true",
      );
      if (trigger !== undefined) {
        window.clearInterval(timer);
        trigger.click();
      } else if (attempts > 1000) {
        window.clearInterval(timer);
      }
    }, 5);
  };
`;
