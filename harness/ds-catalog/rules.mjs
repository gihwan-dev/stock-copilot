export const catalogProblems = (components, samples, entries) => {
  const problems = [];
  for (const component of components) {
    const stories = samples.filter((story) => story.component === component);
    const pages = entries.filter((entry) => entry.component === component);
    if (stories.length === 0) problems.push(`${component}: 이야기 없음`);
    if (pages.length !== 2)
      problems.push(`${component}: 테마 페이지는 정확히 두 개`);
    for (const theme of ["light", "dark"]) {
      if (pages.filter((page) => page.theme === theme).length !== 1)
        problems.push(`${component}: ${theme} 페이지 없음 또는 중복`);
    }
  }
  for (const story of samples) {
    if (!components.includes(story.component)) problems.push("비공개 이야기");
    if (typeof story.render !== "function") problems.push("렌더 함수 없음");
    if (
      story.themes?.length !== 2 ||
      !story.themes.includes("light") ||
      !story.themes.includes("dark")
    )
      problems.push(`${story.component}: 이야기 테마 누락`);
  }
  for (const entry of entries) {
    if (!components.includes(entry.component))
      problems.push("비공개 목록 항목");
    if (typeof entry.render !== "function") problems.push("목록 렌더 없음");
    if (entry.theme !== "light" && entry.theme !== "dark")
      problems.push("목록 테마 불명");
  }
  return problems;
};
