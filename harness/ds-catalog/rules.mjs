export const catalogProblems = (components, samples, entries) => {
  const problems = [];
  for (const component of components) {
    const stories = samples.filter((story) => story.component === component);
    if (stories.length === 0) problems.push(`${component}: 이야기 없음`);
    for (const story of stories) {
      const pages = entries.filter(
        (entry) =>
          entry.component === component && entry.sample === story.sample,
      );
      if (pages.length !== 2)
        problems.push(`${component}: 테마 페이지는 정확히 두 개`);
      for (const theme of ["light", "dark"])
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
    const story = samples.find(
      (story) =>
        story.component === entry.component && story.sample === entry.sample,
    );
    if (story === undefined) problems.push("목록 예제 불명");
    if (entry.title !== entry.component) problems.push("목록 제목 불일치");
    if (entry.observedComponent !== entry.component)
      problems.push("그린 컴포넌트 불일치");
    if (entry.observedTheme !== entry.theme) problems.push("그린 테마 불일치");
    if (entry.observedSample !== entry.sample)
      problems.push("그린 예제 불일치");
    if (entry.observedRender !== story?.render)
      problems.push("예제 렌더 연결 불일치");
  }
  return problems;
};
