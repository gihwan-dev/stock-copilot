import { useState, type ReactElement } from "react";
import { Disclosure, ListRow, Markdown, Stack } from "../../../shared/ds";
import type { ResearchSource } from "../../../shared/ipc";
import {
  parseFreeText,
  type EntityId,
  type ExternalUrl,
  type FreeText,
} from "../../../shared/lib";
import { preserveExcerptPosition } from "../../../shared/platform";
import { researchText } from "../messages";
import { reportParts } from "../model/report-markdown";

type BodyProps = {
  readonly body: FreeText;
  readonly citations: readonly ResearchSource[];
  readonly onOpenLink: (url: ExternalUrl) => void;
  readonly onLocalCitation: (number: EntityId) => void;
};
export const ReportBody = (props: BodyProps): ReactElement => {
  const [state, setState] = useState<"open" | "closed">("closed");
  const parts = reportParts(props.body);
  const render = (
    body: FreeText,
    scale: "lead" | "body",
    appearance: "plain" | "research" | "excerpt" = "plain",
  ): ReactElement => (
    <Markdown
      key={body}
      content={body}
      scale={scale}
      appearance={appearance}
      citations={props.citations}
      linkAllowed={(url) =>
        props.citations.some((source) => source.url === url) ? "allow" : "deny"
      }
      onLocalCitation={props.onLocalCitation}
      onOpenLink={props.onOpenLink}
    />
  );
  if (parts.kind === "sections")
    return (
      <Stack gap="space-2">
        {render(parts.lead, "lead", "research")}
        {render(parts.evidence, "body", "research")}
        {render(parts.conditions, "body", "research")}
      </Stack>
    );
  if (parts.kind === "candidate")
    return (
      <Stack gap="space-2">
        {parts.fields.map((field) => (
          <ListRow
            key={field.role}
            variant="field"
            label={researchText[field.role]}
          >
            {render(field.body, "body", "research")}
          </ListRow>
        ))}
      </Stack>
    );
  if (parts.kind === "document") return render(parts.body, "body");
  const detail = parseFreeText(`${parts.remaining}${researchText.showCount}`);
  return (
    <Disclosure
      title={researchText.remaining}
      detail={detail.kind === "ok" ? detail.value : undefined}
      closeTitle={researchText.collapse}
      open={state}
      onOpenChange={(next, trigger, input): void => {
        preserveExcerptPosition(next, { trigger, input }, setState);
      }}
      preview={render(parts.first, "body")}
    >
      {render(parts.full, "body", "excerpt")}
    </Disclosure>
  );
};
