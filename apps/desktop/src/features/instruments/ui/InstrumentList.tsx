import { useState, type ReactElement } from "react";
import {
  Badge,
  Button,
  DataText,
  EmptyState,
  Inline,
  Percent,
  Skeleton,
  Stack,
  Text,
  TextField,
} from "../../../shared/ds";
import type { InstrumentListRow, ScreenSources } from "../../../shared/ipc";
import {
  emptyInputValue,
  type EntityId,
  type InputValue,
} from "../../../shared/lib";
import { instrumentsText } from "../messages";
import {
  useInstrumentList,
  useWatchlistActions,
  type WatchlistNotice,
} from "../model/instruments";

type ListSources = Pick<
  ScreenSources,
  "instruments" | "addWatchlist" | "removeWatchlist"
>;

const Row = (props: {
  readonly row: InstrumentListRow;
  readonly onSelect: (id: EntityId) => void;
}): ReactElement => (
  <Stack gap="space-1">
    <Inline gap="space-2" align="center" wrap="wrap">
      <Badge>
        {instrumentsText(
          props.row.origin === "holding" ? "holding" : "watchlist",
          "ko",
        )}
      </Badge>
      <DataText value={props.row.name} variant="body-sm" />
      <Percent value={props.row.todayRate} direction={props.row.direction} />
    </Inline>
    <Button
      variant="ghost"
      size="control-height-sm"
      onClick={(): void => {
        props.onSelect(props.row.id);
      }}
    >
      {instrumentsText("open", "ko")}
    </Button>
  </Stack>
);

const Notice = (props: { readonly notice: WatchlistNotice }): ReactElement => {
  switch (props.notice.kind) {
    case "idle":
      return <Stack gap="space-1">{[]}</Stack>;
    case "invalid":
      return (
        <Text variant="caption" tone="alert">
          {instrumentsText("addInvalid", "ko")}
        </Text>
      );
    case "failed":
      return (
        <Text variant="caption" tone="alert">
          {instrumentsText("addFailed", "ko")}
        </Text>
      );
    case "rejected":
      return <DataText value={props.notice.reason} variant="caption" />;
  }
};

const AddForm = (props: { readonly sources: ListSources }): ReactElement => {
  const [value, setValue] = useState<InputValue>(emptyInputValue);
  const actions = useWatchlistActions(props.sources);
  return (
    <Stack gap="space-2">
      <TextField
        label={instrumentsText("addLabel", "ko")}
        value={value}
        onChange={setValue}
      />
      <Button
        size="control-height-sm"
        onClick={(): void => {
          actions.add(value);
        }}
      >
        {instrumentsText("addAction", "ko")}
      </Button>
      <Notice notice={actions.notice} />
    </Stack>
  );
};

const ListBody = (props: {
  readonly sources: ListSources;
  readonly onSelect: (id: EntityId) => void;
}): ReactElement => {
  const list = useInstrumentList(props.sources.instruments);
  switch (list.view.status) {
    case "loading":
      return <Skeleton variant="text" lines={3} />;
    case "empty":
      return (
        <Text variant="caption" tone="ink-muted">
          {instrumentsText("listEmpty", "ko")}
        </Text>
      );
    case "error":
      return (
        <EmptyState
          title={instrumentsText("listError", "ko")}
          action={{
            label: instrumentsText("retry", "ko"),
            onPress: list.retry,
          }}
        />
      );
    case "ready":
      return (
        <Stack gap="space-3">
          {list.view.data.rows.map((row): ReactElement => (
            <Row key={row.id} row={row} onSelect={props.onSelect} />
          ))}
        </Stack>
      );
  }
};

export const InstrumentList = (props: {
  readonly sources: ListSources;
  readonly onSelect: (id: EntityId) => void;
}): ReactElement => (
  <Stack gap="space-4">
    <ListBody sources={props.sources} onSelect={props.onSelect} />
    <AddForm sources={props.sources} />
  </Stack>
);
