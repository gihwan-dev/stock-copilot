import type { ReactElement } from "react";
import {
  Badge,
  DataText,
  Inline,
  Money,
  Percent,
  Probability,
  Quantity,
  Stack,
  Text,
} from "../../../shared/ds";
import type {
  SignalCardView,
  SignalSource,
  SignalSourceKind,
} from "../../../shared/ipc";
import type { UiText } from "../../../shared/lib";
import { signalText, type SignalMessageKey } from "../messages";
import { countedSources, sourceNotice } from "../model/signals";

const sourceKeys = {
  web: "sourceWeb",
  news: "sourceNews",
  disclosure: "sourceDisclosure",
} as const satisfies Record<
  Exclude<SignalSourceKind, "tool">,
  SignalMessageKey
>;

const Fact = (props: {
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

const SizingFacts = (props: { readonly card: SignalCardView }): ReactElement =>
  props.card.sizing.kind === "zero" ? (
    <DataText value={props.card.sizing.reason} variant="body-sm" />
  ) : (
    <Stack gap="space-1">
      <Fact label={signalText("quantity", "ko")}>
        <Quantity value={props.card.sizing.quantity} />
      </Fact>
      <Fact label={signalText("amount", "ko")}>
        <Money value={props.card.sizing.amount} />
      </Fact>
      <Fact label={signalText("costs", "ko")}>
        <Money value={props.card.sizing.costs} />
      </Fact>
      <Fact label={signalText("weightAfter", "ko")}>
        <Percent value={props.card.sizing.weightAfter} />
      </Fact>
    </Stack>
  );

export const Facts = (props: {
  readonly card: SignalCardView;
}): ReactElement => (
  <Stack gap="space-1">
    <Text variant="label">{signalText("factsTitle", "ko")}</Text>
    <Fact label={signalText("price", "ko")}>
      <Money value={props.card.price} />
    </Fact>
    <SizingFacts card={props.card} />
    <Fact label={signalText("aiProbability", "ko")}>
      <Probability value={props.card.aiProbability} />
    </Fact>
    <Fact label={signalText("trackedProbability", "ko")}>
      <Probability value={props.card.trackedProbability} />
    </Fact>
    <Fact label={signalText("threshold", "ko")}>
      <Probability value={props.card.threshold} />
    </Fact>
    <Fact label={signalText("binHitRate", "ko")}>
      {props.card.binHitRate === undefined ? (
        <Text variant="body-sm">{signalText("binHitRateNone", "ko")}</Text>
      ) : (
        <Percent value={props.card.binHitRate} />
      )}
    </Fact>
    <Fact label={signalText("binCount", "ko")}>
      <DataText value={props.card.binCount} variant="body-sm" />
    </Fact>
  </Stack>
);

const SourceRow = (props: { readonly source: SignalSource }): ReactElement => (
  <Inline gap="space-2" align="center" wrap="wrap">
    {props.source.kind === "tool" ? (
      <DataText value={props.source.label} variant="caption" />
    ) : (
      <Badge>{signalText(sourceKeys[props.source.kind], "ko")}</Badge>
    )}
    <DataText value={props.source.label} variant="caption" />
  </Inline>
);

const Sources = (props: {
  readonly sources: readonly SignalSource[];
}): ReactElement => {
  const notice = sourceNotice(props.sources);
  const counted = countedSources(props.sources);
  return (
    <Stack gap="space-1">
      <Text variant="label">{signalText("sources", "ko")}</Text>
      {notice === "none" ? (
        <Text variant="body-sm">{signalText("noSources", "ko")}</Text>
      ) : (
        <Stack gap="space-1">
          {counted.map((source): ReactElement => (
            <SourceRow key={source.id} source={source} />
          ))}
        </Stack>
      )}
      {notice === "external-only" ? (
        <Text variant="body-sm" tone="alert">
          {signalText("externalOnly", "ko")}
        </Text>
      ) : (
        <Stack gap="space-1">{[]}</Stack>
      )}
    </Stack>
  );
};

export const Reasons = (props: {
  readonly card: SignalCardView;
}): ReactElement => (
  <Stack gap="space-2">
    <Text variant="label">{signalText("reasonsTitle", "ko")}</Text>
    <Fact label={signalText("rationale", "ko")}>
      <DataText value={props.card.rationale} variant="body-sm" />
    </Fact>
    <Fact label={signalText("counterargument", "ko")}>
      <DataText value={props.card.counterargument} variant="body-sm" />
    </Fact>
    <Sources sources={props.card.sources} />
  </Stack>
);
