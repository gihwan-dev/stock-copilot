export const catalogSamples = (sources) => {
  const positions = new Map();
  return Object.keys(sources)
    .sort()
    .flatMap((key) => sources[key])
    .filter((story) => !("theme" in story) || story.theme === "light")
    .map((story) => {
      const sample = positions.get(story.component) ?? 0;
      positions.set(story.component, sample + 1);
      return { ...story, sample };
    });
};
