import { useMutation, useQuery } from "@tanstack/react-query";
import type { ReactElement } from "react";
import {
  Button,
  Card,
  DataText,
  Inline,
  Markdown,
  Stack,
  Text,
  TimestampText,
} from "../../../shared/ds";
import {
  getDashboardResearch,
  openExternal,
  type ResearchSection,
  type ResearchSource,
} from "../../../shared/ipc";
import type { EntityId } from "../../../shared/lib";
import { focusResearchSection } from "../../../shared/platform";
import { researchText } from "../messages";

const topics = ["holdings", "candidates", "economy"] as const;

const Source = ({ item }: { readonly item: ResearchSource }): ReactElement => {
  const link = useMutation({ mutationFn: openExternal });
  return (
    <Inline gap="space-2" align="center" wrap="wrap">
      <DataText value={item.label} variant="code" />
      <DataText value={item.title} />
      {item.url === null ? (
        []
      ) : (
        <Button
          variant="ghost"
          size="control-height-sm"
          onClick={() => {
            if (item.url !== null) link.mutate(item.url);
          }}
        >
          {researchText.open}
        </Button>
      )}
      {item.observedAt === null ? (
        []
      ) : (
        <TimestampText value={item.observedAt} variant="caption" />
      )}
    </Inline>
  );
};

const InstrumentAction = (props: {
  readonly symbol: EntityId;
  readonly open: (symbol: EntityId) => void;
}): ReactElement => (
  <Button
    size="control-height-sm"
    onClick={(): void => {
      props.open(props.symbol);
    }}
  >
    {researchText.detail}
  </Button>
);

const SourceList = ({
  report,
  topic,
}: {
  readonly report: ResearchSection;
  readonly topic: Topic;
}): ReactElement => (
  <Stack gap="space-1">
    {report.sources.length === 0 ? (
      []
    ) : (
      <Text variant="label">{researchText.sources}</Text>
    )}
    {report.sources.map((item) => (
      <Stack key={item.number} position={{ topic, source: item.number }}>
        <Source item={item} />
      </Stack>
    ))}
  </Stack>
);

const Report = ({
  report,
  topic,
  open,
}: {
  readonly topic: Topic;
  readonly open: (symbol: EntityId) => void;
  readonly report: ResearchSection;
}): ReactElement => {
  const link = useMutation({ mutationFn: openExternal });
  const citations = report.sources;
  return (
    <Stack gap="space-2">
      {report.status === "ready" ? (
        []
      ) : (
        <Text tone="ink-muted">{researchText[report.status]}</Text>
      )}
      {report.entries.map((entry) => (
        <Stack key={entry.symbol ?? entry.body} gap="space-1">
          {entry.symbol === null ? (
            []
          ) : (
            <Inline gap="space-2" align="center" wrap="wrap">
              {entry.displayName === null ? (
                []
              ) : (
                <DataText value={entry.displayName} variant="emphasis" />
              )}
              <DataText value={entry.symbol} variant="code" />
              {topic === "candidates" && entry.detail === "available" ? (
                <InstrumentAction symbol={entry.symbol} open={open} />
              ) : (
                []
              )}
            </Inline>
          )}
          <Markdown
            content={entry.body}
            citations={citations}
            onLocalCitation={(number) => {
              focusResearchSection(topic, number);
            }}
            linkAllowed={(url) =>
              citations.some((item) => item.url === url) ? "allow" : "deny"
            }
            onOpenLink={link.mutate}
          />
        </Stack>
      ))}
      <SourceList report={report} topic={topic} />
    </Stack>
  );
};

type Topic = "holdings" | "candidates" | "economy";
export const useDashboardResearch = (
  open: (symbol: EntityId) => void,
): ((topic: Topic) => ReactElement) => {
  const query = useQuery({
    queryKey: ["dashboard-research"],
    queryFn: getDashboardResearch,
    refetchInterval: (current) => {
      const result = current.state.data;
      return result?.kind === "ok" &&
        topics.some((topic) => result.value[topic].status === "refreshing")
        ? 5000
        : 300_000;
    },
  });
  const result = query.data;
  return (topic): ReactElement => {
    const at = result?.kind === "ok" ? result.value[topic].asOf : null;
    return (
      <Card
        position={{ topic }}
        title={researchText[topic]}
        meta={
          at === null ? undefined : (
            <Inline gap="space-1">
              <Text variant="caption">{researchText.asOf}</Text>
              <TimestampText value={at} />
            </Inline>
          )
        }
      >
        {result?.kind === "ok" ? (
          <Report topic={topic} open={open} report={result.value[topic]} />
        ) : (
          <Text>
            {query.isPending ? researchText.loading : researchText.failed}
          </Text>
        )}
      </Card>
    );
  };
};

export const ResearchNavigation = (): ReactElement => (
  <Inline gap="space-4" wrap="wrap">
    {topics.map((topic) => (
      <Button
        key={topic}
        variant="link"
        size="control-height-sm"
        onClick={(): void => {
          focusResearchSection(topic);
        }}
      >
        {researchText[topic]}
      </Button>
    ))}
  </Inline>
);
