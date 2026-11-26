import type { ReactElement } from "react";
import {
  Button,
  CandleChart,
  Card,
  DataText,
  EmptyState,
  Inline,
  Money,
  Percent,
  Quantity,
  Skeleton,
  Stack,
  Text,
  TimestampText,
  type CandlePoint,
} from "../../../shared/ds";
import type { InstrumentDetailView, ScreenSources } from "../../../shared/ipc";
import type { EntityId, UiText } from "../../../shared/lib";
import { useCanvasChartTheme, useColorScheme } from "../../../shared/platform";
import { instrumentsText } from "../messages";
import { useInstrumentDetail, useWatchlistActions } from "../model/instruments";
import { RecentNews } from "./RecentNews";
import { ThesisNotes } from "./ThesisNotes";

type DetailSources = Pick<
  ScreenSources,
  "instrumentDetail" | "thesis" | "addWatchlist" | "removeWatchlist"
>;

const Line = (props: {
  readonly label: UiText;
  readonly children: ReactElement;
}): ReactElement => (
  <Inline gap="space-2" align="center" wrap="wrap">
    <Text variant="caption" tone="ink-muted">
      {props.label}
    </Text>
    {props.children}
  </Inline>
);

const Chart = (props: {
  readonly view: InstrumentDetailView;
}): ReactElement => {
  const scheme = useColorScheme();
  const palette = useCanvasChartTheme(scheme);
  const points: readonly CandlePoint[] = props.view.candles.map(
    (candle): CandlePoint => ({
      id: candle.id,
      label: candle.label,
      open: candle.open,
      high: candle.high,
      low: candle.low,
      close: candle.close,
    }),
  );
  return (
    <Card title={instrumentsText("chartTitle", "ko")}>
      {palette === undefined ? (
        <CandleChart
          data={points}
          label={instrumentsText("chartLabel", "ko")}
          empty={instrumentsText("chartEmpty", "ko")}
          theme={scheme}
        />
      ) : (
        <CandleChart
          data={points}
          label={instrumentsText("chartLabel", "ko")}
          empty={instrumentsText("chartEmpty", "ko")}
          theme={scheme}
          palette={palette}
        />
      )}
    </Card>
  );
};

const Holding = (props: {
  readonly view: InstrumentDetailView;
}): ReactElement => (
  <Card title={instrumentsText("holdingTitle", "ko")}>
    {props.view.holding === undefined ? (
      <Text variant="body-sm">{instrumentsText("holdingNone", "ko")}</Text>
    ) : (
      <Stack gap="space-1">
        <Line label={instrumentsText("quantity", "ko")}>
          <Quantity value={props.view.holding.quantity} />
        </Line>
        <Line label={instrumentsText("value", "ko")}>
          <Money value={props.view.holding.value} />
        </Line>
        <Line label={instrumentsText("profit", "ko")}>
          <Inline gap="space-2" align="center">
            <Money value={props.view.holding.profit} />
            <Percent value={props.view.holding.profitRate} />
          </Inline>
        </Line>
      </Stack>
    )}
  </Card>
);

const Ready = (props: {
  readonly view: InstrumentDetailView;
  readonly sources: DetailSources;
  readonly onClose: () => void;
}): ReactElement => {
  const actions = useWatchlistActions(props.sources);
  return (
    <Stack gap="space-4">
      <Card
        title={instrumentsText("quoteTitle", "ko")}
        actions={{
          label: instrumentsText("close", "ko"),
          onPress: props.onClose,
        }}
      >
        <Stack gap="space-1">
          <Inline gap="space-2" align="center" wrap="wrap">
            <DataText value={props.view.name} />
            <DataText value={props.view.ticker} variant="code" />
          </Inline>
          <Line label={instrumentsText("price", "ko")}>
            <Money value={props.view.price} />
          </Line>
          <Line label={instrumentsText("today", "ko")}>
            {props.view.todayRate === undefined ? (
              <Text variant="caption" tone="ink-muted">
                {instrumentsText("todayUnknown", "ko")}
              </Text>
            ) : (
              <Percent
                value={props.view.todayRate}
                direction={props.view.direction}
              />
            )}
          </Line>
          <Line label={instrumentsText("quotedAt", "ko")}>
            <TimestampText value={props.view.quotedAt} variant="caption" />
          </Line>
        </Stack>
      </Card>
      <Chart view={props.view} />
      <Holding view={props.view} />
      <RecentNews news={props.view.news ?? []} />
      <ThesisNotes load={props.sources.thesis} instrument={props.view.id} />
      <Button
        variant="ghost"
        onClick={(): void => {
          actions.remove(props.view.id);
        }}
      >
        {instrumentsText("removeAction", "ko")}
      </Button>
    </Stack>
  );
};

export const InstrumentDetail = (props: {
  readonly sources: DetailSources;
  readonly instrument: EntityId;
  readonly onClose: () => void;
}): ReactElement => {
  const detail = useInstrumentDetail(
    props.sources.instrumentDetail,
    props.instrument,
  );
  const close = {
    label: instrumentsText("close", "ko"),
    onPress: props.onClose,
  };
  switch (detail.view.status) {
    case "loading":
      return (
        <Card title={instrumentsText("detailLoading", "ko")} actions={close}>
          <Skeleton variant="text" lines={3} />
        </Card>
      );
    case "empty":
      return (
        <EmptyState
          title={instrumentsText("detailEmpty", "ko")}
          action={close}
        />
      );
    case "error":
      return (
        <EmptyState
          title={instrumentsText("detailError", "ko")}
          action={{
            label: instrumentsText("retry", "ko"),
            onPress: detail.retry,
          }}
        />
      );
    case "ready":
      return (
        <Ready
          view={detail.view.data}
          sources={props.sources}
          onClose={props.onClose}
        />
      );
  }
};
