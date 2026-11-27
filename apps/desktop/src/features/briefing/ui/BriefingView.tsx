import { useState, type ComponentProps, type ReactElement } from "react";
import { Button, Card, Inline, Stack } from "../../../shared/ds";
import type { BriefingSummary } from "../../../shared/ipc";
import type { ExternalUrl } from "../../../shared/lib";
import { briefingText } from "../messages";
import { Body, Reader, Stamp } from "./BriefingReader";

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
type ReaderState = "open" | "closed";

export const ReadingCard = ({
  view: props,
  reader,
  changeReader,
}: {
  readonly view: BriefingViewProps;
  readonly reader: ReaderState;
  readonly changeReader: (next: ReaderState) => void;
}): ReactElement<ComponentProps<typeof Card>> => {
  const close = (): void => {
    changeReader("closed");
  };
  const history = props.onOpenHistory;
  return (
    <Card
      title={briefingText(props.briefing.kind, "ko")}
      layout={props.layout ?? "flow"}
      actions={{
        label: briefingText("openReader", "ko"),
        onPress: () => {
          changeReader("open");
        },
      }}
    >
      <Stack gap="space-3">
        <CardStamp briefing={props.briefing} history={history} />
        <Body
          briefing={props.briefing}
          onOpenSource={props.onOpenSource}
          scale="body"
        />
        {reader === "open"
          ? [
              <Reader
                key="reader"
                briefing={props.briefing}
                onOpenSource={props.onOpenSource}
                onOpenHistory={
                  history === undefined
                    ? undefined
                    : (): void => {
                        close();
                        history();
                      }
                }
                onClose={close}
              />,
            ]
          : []}
      </Stack>
    </Card>
  );
};

export const BriefingView = (props: BriefingViewProps): ReactElement => {
  const [reader, setReader] = useState<ReaderState>("closed");
  return (
    <ReadingCard view={props} reader={reader} changeReader={setReader} />
  );
};
