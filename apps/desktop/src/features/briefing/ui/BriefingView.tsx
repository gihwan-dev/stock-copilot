import type { ReactElement } from "react";
import { Button, Card, Inline, Stack } from "../../../shared/ds";
import type { BriefingSummary } from "../../../shared/ipc";
import type { ExternalUrl } from "../../../shared/lib";
import { briefingText } from "../messages";
import { Body, Stamp } from "./BriefingReader";

const CardStamp = (props: {
  readonly briefing: BriefingSummary;
  readonly history: (() => void) | undefined;
}): ReactElement => (
  <Inline gap="space-2" align="center" justify="between" wrap="wrap">
    {[
      <Stamp key="stamp" briefing={props.briefing} />,
      ...(props.history === undefined
        ? []
        : [
            <Button
              key="history"
              variant="ghost"
              size="control-height-sm"
              onClick={props.history}
            >
              {briefingText("openHistory", "ko")}
            </Button>,
          ]),
    ]}
  </Inline>
);

export type BriefingLayout = "flow" | "fill";

type BriefingViewProps = {
  readonly briefing: BriefingSummary;
  readonly onOpenSource: (url: ExternalUrl) => void;
  readonly onOpenHistory: (() => void) | undefined;
  readonly layout?: BriefingLayout;
};

export const BriefingView = (props: BriefingViewProps): ReactElement => (
  <Card
    title={briefingText(props.briefing.kind, "ko")}
    layout={props.layout ?? "flow"}
  >
    <Stack gap="space-3">
      <CardStamp briefing={props.briefing} history={props.onOpenHistory} />
      <Body
        briefing={props.briefing}
        onOpenSource={props.onOpenSource}
        presentation="full"
      />
    </Stack>
  </Card>
);
