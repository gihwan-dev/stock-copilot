import { parseFreeText, type FreeText } from "../../../shared/lib";

const citedLine = /\[[1-9]\d*\][.。]?$/u;
const blockMarkup =
  /^(?: {4}|\t| {0,3}(?:#{1,6}(?:\s|$)|[>|`~]|[-*+]\s|\d+[.)]\s|<|\[.+\]:))/u;
const candidateLabel = /^(이유|위험|확인 조건)(?:[：:]\s*|\s+)/u;

export const reportMarkdown = (body: FreeText): FreeText => {
  const lines = body.trimEnd().split(/\r?\n/u);
  if (
    lines.length < 2 ||
    lines.some(
      (line) => blockMarkup.test(line) || !citedLine.test(line.trimEnd()),
    )
  )
    return body;
  const candidate = lines.every((line) => candidateLabel.test(line));
  const rendered = lines.map((line) =>
    candidate ? line.replace(candidateLabel, "**$1** ") : `- ${line}`,
  );
  const parsed = parseFreeText(rendered.join(candidate ? "\n\n" : "\n"));
  return parsed.kind === "ok" ? parsed.value : body;
};
