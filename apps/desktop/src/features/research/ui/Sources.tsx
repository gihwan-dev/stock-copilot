import type { ReactElement } from "react";
import { useMutation } from "@tanstack/react-query";
import {
  Button,
  DataText,
  Disclosure,
  Inline,
  Stack,
  TimestampText,
} from "../../../shared/ds";
import {
  openExternal,
  type ResearchSection,
  type ResearchSource,
} from "../../../shared/ipc";
import { parseFreeText, type FreeText } from "../../../shared/lib";
import { researchText } from "../messages";

export type ListState = "closed" | "open";
type Topic = "holdings" | "candidates" | "economy";

const Source = ({
  item,
  last,
}: {
  readonly item: ResearchSource;
  readonly last: ResearchSource;
}): ReactElement => {
  const link = useMutation({ mutationFn: openExternal });
  const number = parseFreeText(
    item.label.padStart(last.label.length, "\u2007"),
  );
  return (
    <Inline gap="space-2" align="center" wrap="wrap">
      <DataText
        value={number.kind === "ok" ? number.value : item.label}
        variant="code"
      />
      <DataText value={item.title} />
      {item.observedAt === null ? (
        []
      ) : (
        <TimestampText value={item.observedAt} variant="caption" />
      )}
      {item.url === null ? (
        []
      ) : (
        <Button
          variant="link"
          aria-label={researchText.open}
          size="control-height-sm"
          onClick={() => {
            if (item.url !== null) link.mutate(item.url);
          }}
        >
          {researchText.openExternal}
        </Button>
      )}
    </Inline>
  );
};

const Rows = (props: {
  readonly items: readonly ResearchSource[];
  readonly topic: Topic;
  readonly last: ResearchSource;
}): ReactElement => (
  <Stack gap="space-1">
    {props.items.map((item) => (
      <Stack
        key={item.number}
        position={{ topic: props.topic, source: item.number }}
      >
        <Source item={item} last={props.last} />
      </Stack>
    ))}
  </Stack>
);

const count = (items: readonly ResearchSource[]): FreeText | undefined => {
  const text = parseFreeText(
    `${items.length.toString()}${researchText.countUnit}`,
  );
  return text.kind === "ok" ? text.value : undefined;
};

export const Sources = (props: {
  readonly report: ResearchSection;
  readonly topic: Topic;
  readonly state: ListState;
  readonly change: (state: ListState) => void;
}): ReactElement | null => {
  const { entries, sources } = props.report;
  const last = sources.at(-1);
  if (last === undefined) return null;
  const cited = sources.filter((source) =>
    entries.some((entry) => entry.body.includes(source.label)),
  );
  const additional =
    cited.length === 0
      ? []
      : sources.filter((source) => !cited.includes(source));
  return (
    <Disclosure
      title={researchText.sources}
      detail={count(sources)}
      open={props.state}
      onOpenChange={props.change}
    >
      <Stack gap="space-2">
        <Rows
          items={cited.length === 0 ? sources : cited}
          last={last}
          topic={props.topic}
        />
        {additional.length === 0 ? (
          []
        ) : (
          <Disclosure
            title={researchText.additional}
            detail={count(additional)}
          >
            <Rows items={additional} topic={props.topic} last={last} />
          </Disclosure>
        )}
      </Stack>
    </Disclosure>
  );
};
