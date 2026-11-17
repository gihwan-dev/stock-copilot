import { createRequire } from "node:module";

const require = createRequire(
  new URL("../../apps/desktop/package.json", import.meta.url),
);
const { isValidElement } = require("react");
const { renderToStaticMarkup } = require("react-dom/server");

export const stressProblems = (components, samples) =>
  components
    .filter(
      (component) =>
        !samples.some(
          (story) => story.component === component && story.state === "stress",
        ),
    )
    .map((component) => `${component}: 부하 이야기 없음`);

export const stressRenderProblems = (samples) =>
  samples.flatMap((story) =>
    ["light", "dark"]
      .filter(
        (theme) =>
          typeof story.render !== "function" ||
          !isValidElement(story.render(theme)),
      )
      .map((theme) => `${story.component}: ${theme} 부하 이야기 렌더 없음`),
  );

export const stressFixtureProblems = (fixtures, samples) => {
  const problems = [];
  if (
    !Array.isArray(fixtures.holdings) ||
    fixtures.holdings.length !== 200 ||
    new Set(fixtures.holdings).size !== 200
  )
    problems.push("보유 종목 부하 자료는 서로 다른 200행이어야 함");
  if (
    !Array.isArray(fixtures.paragraphs) ||
    fixtures.paragraphs.length !== 20 ||
    fixtures.paragraphs.some((paragraph) => paragraph.length === 0)
  )
    problems.push("근거 부하 자료는 비어 있지 않은 20문단이어야 함");
  if (
    !Array.isArray(fixtures.sources) ||
    fixtures.sources.length !== 30 ||
    new Set(fixtures.sources).size !== 30
  )
    problems.push("출처 부하 자료는 서로 다른 30개여야 함");
  const table = samples.find((story) => story.component === "Table");
  const tableElement = table?.render?.("light");
  if (
    !isValidElement(tableElement) ||
    (renderToStaticMarkup(tableElement).match(/<tr(?:\s|>)/gu) ?? []).length !==
      201
  )
    problems.push("보유 종목 부하 자료의 표 렌더는 200행이어야 함");
  const markdown = samples.find((story) => story.component === "Markdown");
  const markdownElement = markdown?.render?.("light");
  const paragraphs = Array.isArray(fixtures.paragraphs)
    ? fixtures.paragraphs
    : [];
  const sources = Array.isArray(fixtures.sources) ? fixtures.sources : [];
  const expectedMarkdown = [
    ...paragraphs,
    ...sources.map((source) => `[${fixtures.english}](${source})`),
  ].join("\n\n");
  if (
    !isValidElement(markdownElement) ||
    markdownElement.props.content !== expectedMarkdown
  )
    problems.push("Markdown 부하 자료의 문단과 출처가 누락됨");
  return problems;
};

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
    if (!entry.tags?.includes("dev")) problems.push("목록에서 숨긴 이야기");
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
