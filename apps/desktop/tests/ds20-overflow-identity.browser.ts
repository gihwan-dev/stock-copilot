export const identifyOverflowElement = (element: Readonly<Element>): string => {
  const parts: string[] = [];
  let current: Readonly<Element> | null = element;
  while (current !== null) {
    const parent: Readonly<Element> | null = current.parentElement;
    const siblings = parent === null ? [current] : [...parent.children];
    parts.unshift(
      `${current.tagName.toLowerCase()}:nth-child(${siblings.indexOf(current) + 1})`,
    );
    current = parent;
  }
  const label = `${element.tagName.toLowerCase()}${element.id === "" ? "" : `#${element.id}`}`;
  return `${label} @ ${parts.join(" > ")}`;
};
