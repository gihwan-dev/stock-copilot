import type { ReactElement } from "react";
import { DataText, Stack, Text } from "../../../shared/ds";
import type { RiskSaveOutcome } from "../../../shared/ipc";
import { riskText, type RiskMessageKey } from "../messages";
import type { SaveNotice as Notice } from "../model/risk";

const plain = {
  saved: { key: "risk.saved", tone: "ink" },
  unchanged: { key: "risk.unchanged", tone: "ink" },
  declined: { key: "risk.declined", tone: "alert" },
  unavailable: { key: "risk.unavailable", tone: "alert" },
  failed: { key: "risk.saveFailed", tone: "alert" },
  rejected: { key: "risk.saveFailed", tone: "alert" },
} as const satisfies Record<
  Exclude<Notice, "idle">,
  { readonly key: RiskMessageKey; readonly tone: "ink" | "alert" }
>;

export const SaveNotice = (props: {
  readonly notice: Notice;
  readonly outcome: RiskSaveOutcome | undefined;
}): ReactElement => {
  if (props.notice === "idle") return <Stack gap="space-1">{[]}</Stack>;
  if (props.outcome?.kind === "rejected")
    return <DataText value={props.outcome.reason} variant="body-sm" />;
  const entry = plain[props.notice];
  return (
    <Text variant="body-sm" tone={entry.tone}>
      {riskText(entry.key, "ko")}
    </Text>
  );
};
