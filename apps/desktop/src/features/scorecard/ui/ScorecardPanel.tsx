import { useQuery, useQueryClient } from "@tanstack/react-query";
import type { ReactElement } from "react";
import {
  DataText,
  Disclosure,
  EmptyState,
  Inline,
  Skeleton,
  Stack,
  Text,
} from "../../../shared/ds";
import {
  toScreenState,
  type ScorecardScreenView,
  type ScreenSources,
} from "../../../shared/ipc";
import { queryKeys, screenKeys } from "../../../shared/lib";
import { detach } from "../../../shared/platform";
import { scorecardText } from "../messages";
import { CombinationView } from "./CombinationView";

const Ready = (props: { readonly view: ScorecardScreenView }): ReactElement => (
  <Stack gap="space-6">
    {props.view.combinations.map((combination): ReactElement => (
      <Stack key={combination.id} gap="space-3">
        <Inline gap="space-2" align="center" wrap="wrap">
          <Text variant="label">
            {scorecardText("scorecard.combination", "ko")}
          </Text>
          <DataText value={combination.model} />
          <DataText value={combination.horizon} />
        </Inline>
        <CombinationView combination={combination} />
      </Stack>
    ))}
  </Stack>
);

const Body = (props: {
  readonly load: ScreenSources["scorecard"];
}): ReactElement => {
  const client = useQueryClient();
  const query = useQuery({
    queryKey: screenKeys.scorecard,
    queryFn: props.load,
  });
  const view = toScreenState(query.data, (data) =>
    data.combinations.length === 0 ? "empty" : "filled",
  );
  switch (view.status) {
    case "loading":
      return <Skeleton variant="text" lines={3} />;
    case "empty":
      return <EmptyState title={scorecardText("scorecard.empty", "ko")} />;
    case "error":
      return (
        <EmptyState
          title={scorecardText("scorecard.error", "ko")}
          action={{
            label: scorecardText("scorecard.retry", "ko"),
            onPress: (): void => {
              detach({
                kind: "invalidate-queries",
                client,
                keys: [queryKeys.scorecard],
              });
            },
          }}
        />
      );
    case "ready":
      return <Ready view={view.data} />;
  }
};

export const ScorecardPanel = (props: {
  readonly load: ScreenSources["scorecard"];
}): ReactElement => (
  <Disclosure title={scorecardText("scorecard.title", "ko")}>
    <Body load={props.load} />
  </Disclosure>
);
