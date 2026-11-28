import { parseFreeText, type FreeText } from "../../../shared/lib";

const citedLine = /\[[1-9]\d*\][.。]?$/u;
const blockMarkup =
  /^(?: {4}|\t| {0,3}(?:#{1,6}(?:\s|$)|[>|`~]|[-*+]\s|\d+[.)]\s|<|\[.+\]:))/u;
const candidateLabel = /^(이유|위험|확인 조건)(?:[：:]\s*|\s+)/u;

type CandidateField = {
  readonly role: "reason" | "risk" | "condition";
  readonly body: FreeText;
};
type Candidate = {
  readonly kind: "candidate";
  readonly fields: readonly CandidateField[];
};
const labels = [
  { role: "reason", prefix: /^이유(?:[：:]|\s)/u },
  { role: "risk", prefix: /^위험(?:[：:]|\s)/u },
  { role: "condition", prefix: /^확인 조건(?:[：:]|\s)/u },
] as const;
const candidateParts = (body: FreeText): Candidate | undefined => {
  const lines = body
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
  if (lines.length !== labels.length) return undefined;
  const fields = lines.flatMap((line, i) => {
    const match = candidateLabel.exec(line);
    const label = labels.at(i);
    if (
      match === null ||
      label === undefined ||
      !label.prefix.test(line) ||
      !citedLine.test(line)
    )
      return [];
    const body = parseFreeText(line.slice(match[0].length));
    return body.kind === "ok" ? [{ role: label.role, body: body.value }] : [];
  });
  return fields.length === labels.length
    ? { kind: "candidate", fields }
    : undefined;
};

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

type Excerpt = {
  readonly kind: "excerpt";
  readonly first: FreeText;
  readonly full: FreeText;
  readonly remaining: FreeText;
};
type Sections = {
  readonly kind: "sections";
  readonly lead: FreeText;
  readonly evidence: FreeText;
  readonly conditions: FreeText;
};
type Document = { readonly kind: "document"; readonly body: FreeText };
const previewSize = 6;
const excerpt = (markdown: FreeText): Excerpt | undefined => {
  const lines = markdown.split("\n");
  if (
    lines.length <= previewSize ||
    !lines.every((line) => line.startsWith("- "))
  )
    return undefined;
  const first = parseFreeText(lines.slice(0, previewSize).join("\n"));
  const remaining = parseFreeText((lines.length - previewSize).toString());
  if (first.kind === "ok" && remaining.kind === "ok")
    return {
      kind: "excerpt",
      first: first.value,
      full: markdown,
      remaining: remaining.value,
    };
  return undefined;
};

const sections = (body: FreeText): Sections | undefined => {
  const match =
    /^(### 핵심\s*\n[\s\S]+?)\n(### 판단 근거\s*\n[\s\S]+?)\n(### 확인 조건\s*\n[\s\S]+)$/u.exec(
      body.trim(),
    );
  if (match === null) return undefined;
  const lead = parseFreeText(match[1]?.trim());
  const evidence = parseFreeText(match[2]?.trim());
  const conditions = parseFreeText(match[3]?.trim());
  return lead.kind === "ok" &&
    evidence.kind === "ok" &&
    conditions.kind === "ok"
    ? {
        kind: "sections",
        lead: lead.value,
        evidence: evidence.value,
        conditions: conditions.value,
      }
    : undefined;
};
export const reportParts = (
  body: FreeText,
): Excerpt | Sections | Candidate | Document => {
  const candidate = candidateParts(body);
  if (candidate !== undefined) return candidate;
  const markdown = reportMarkdown(body);
  const folded = markdown === body ? undefined : excerpt(markdown);
  return folded ?? sections(markdown) ?? { kind: "document", body: markdown };
};
