import { useMutation, useQuery } from "@tanstack/react-query";
import { useState, type ReactElement } from "react";
import {
  Button,
  Card,
  DataText,
  Inline,
  Stack,
  Text,
  TimestampText,
} from "../../../shared/ds";
import {
  getDashboardResearch,
  openExternal,
  type ResearchSection,
} from "../../../shared/ipc";
import type { EntityId } from "../../../shared/lib";
import {
  focusResearchSection,
  useWindowLayout,
} from "../../../shared/platform";
import { ReportBody } from "./ReportBody";
import { Sources, type ListState } from "./Sources";
import { researchText } from "../messages";

const topics = ["holdings", "candidates", "economy"] as const;

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

type ReportProps = {
  readonly topic: Topic;
  readonly open: (symbol: EntityId) => void;
  readonly report: ResearchSection;
};

const Report = ({ report, topic, open }: ReportProps): ReactElement => {
  const link = useMutation({ mutationFn: openExternal });
  const citations = report.sources;
  const [sourceList, setSourceList] = useState<ListState>("closed");
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
          <ReportBody
            body={entry.body}
            citations={citations}
            onLocalCitation={(number) => {
              focusResearchSection(topic, number, () => {
                setSourceList("open");
              });
            }}
            onOpenLink={link.mutate}
          />
        </Stack>
      ))}
      <Sources
        report={report}
        topic={topic}
        state={sourceList}
        change={setSourceList}
      />
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

const compactTopics = [
  "positions",
  "daily-briefing",
  "signals",
  ...topics,
] as const;
const signalTopics = ["signals", ...topics] as const;
const navigationTopics = {
  compact: compactTopics,
  "summary-row": signalTopics,
  "two-columns": signalTopics,
  "three-columns": topics,
} as const;
export const ResearchNavigation = (props: {
  readonly account: "ready" | "pending";
}): ReactElement => {
  const layout = useWindowLayout();
  const destinations =
    props.account === "ready" ? navigationTopics[layout] : topics;
  return (
    <Inline gap="space-4" wrap="wrap">
      {destinations.map((topic) => (
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
};
