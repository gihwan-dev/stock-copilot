import type { ReactElement } from "react";
import { Text } from "../../../shared/ds";
import type { OrderRequestOutcome } from "../../../shared/ipc";
import { signalText, type SignalMessageKey } from "../messages";

const outcomeKeys = {
  requested: "outcomeRequested",
  "outcome-unknown": "outcomeUnknown",
  "presence-denied": "outcomeDenied",
  "switch-off": "outcomeSwitchOff",
  "hourly-limit": "outcomeHourly",
  "market-closed": "outcomeClosed",
  "dialog-in-progress": "outcomeBusy",
  cooldown: "outcomeCooldown",
  "already-consumed": "outcomeConsumed",
  "proposal-unavailable": "outcomeGone",
  "conditions-changed": "outcomeChanged",
  "high-value-rejected": "outcomeHighValue",
  "broker-rejected": "outcomeBroker",
  unavailable: "outcomeUnavailable",
  other: "outcomeOther",
} as const satisfies Record<OrderRequestOutcome, SignalMessageKey>;

export const OutcomeNotice = (props: {
  readonly outcome: OrderRequestOutcome;
}): ReactElement => (
  <Text
    variant="body-sm"
    tone={props.outcome === "requested" ? "ink" : "alert"}
  >
    {signalText(outcomeKeys[props.outcome], "ko")}
  </Text>
);
