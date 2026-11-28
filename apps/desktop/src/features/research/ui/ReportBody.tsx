import { useState, type ReactElement } from "react";
import { Disclosure, Markdown, Stack } from "../../../shared/ds";
import type { ResearchSource } from "../../../shared/ipc";
import {
  parseFreeText,
  type EntityId,
  type ExternalUrl,
  type FreeText,
} from "../../../shared/lib";
import { researchText } from "../messages";
import { reportParts } from "../model/report-markdown";

export const ReportBody = (props: {
  readonly body: FreeText;
  readonly citations: readonly ResearchSource[];
  readonly onOpenLink: (url: ExternalUrl) => void;
  readonly onLocalCitation: (number: EntityId) => void;
}): ReactElement => {
  const [state, setState] = useState<"open" | "closed">("closed");
  const parts = reportParts(props.body);
  const render = (body: FreeText, scale: "lead" | "body"): ReactElement => (
    <Markdown
      key={body}
      content={body}
      scale={scale}
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
      <Stack gap="space-4">
        {render(parts.lead, "lead")}
        {render(parts.evidence, "body")}
        {render(parts.conditions, "body")}
      </Stack>
    );
  if (parts.kind === "document") return render(parts.body, "body");
  const detail = parseFreeText(
    `${parts.remaining}${state === "open" ? researchText.hideCount : researchText.showCount}`,
  );
  return (
    <Stack gap="space-1">
      {render(parts.first, "body")}
      <Disclosure
        title={researchText.remaining}
        detail={detail.kind === "ok" ? detail.value : undefined}
        open={state}
        onOpenChange={setState}
      >
        {render(parts.rest, "body")}
      </Disclosure>
    </Stack>
  );
};
