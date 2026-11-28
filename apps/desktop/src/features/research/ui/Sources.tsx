import type { ReactElement } from "react";
import { useMutation } from "@tanstack/react-query";
import { ListRow, Disclosure, Stack } from "../../../shared/ds";
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
  const url = item.url;
  return (
    <ListRow
      variant="reference"
      number={number.kind === "ok" ? number.value : item.label}
      sourceTitle={item.title}
      observedAt={item.observedAt}
      linkLabel={researchText.openExternal}
      onPress={
        url === null
          ? undefined
          : (): void => {
              link.mutate(url);
            }
      }
    />
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
  const tally =
    additional.length === 0
      ? undefined
      : parseFreeText(
          `${researchText.cited} ${cited.length.toString()}${researchText.countUnit} / ${researchText.total} ${sources.length.toString()}${researchText.countUnit}`,
        );
  return (
    <Disclosure
      title={researchText.sources}
      detail={tally?.kind === "ok" ? tally.value : count(sources)}
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
