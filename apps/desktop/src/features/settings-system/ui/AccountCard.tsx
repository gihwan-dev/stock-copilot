import type { ReactElement } from "react";
import { Card, Select, Stack, Text, TextField } from "../../../shared/ds";
import type { AccountRef, AccountSelection } from "../../../shared/ipc";
import {
  parseInputValue,
  type InputValue,
  type UiText,
} from "../../../shared/lib";
import { systemText, type SystemMessageKey } from "../messages";
import { useAccountChange, useAccounts } from "../model";
import { useSystemClient } from "./client";

const ordinals: readonly SystemMessageKey[] = [
  "system.account1",
  "system.account2",
  "system.account3",
  "system.account4",
  "system.account5",
];

type Choice = {
  readonly id: AccountRef;
  readonly value: InputValue;
  readonly label: UiText;
  readonly number: InputValue;
};

const choicesOf = (selection: AccountSelection): readonly Choice[] =>
  selection.accounts.flatMap((account, index) => {
    const key = ordinals.at(index);
    const value = parseInputValue(account.id);
    const number = parseInputValue(account.number);
    if (key === undefined || value.kind === "err" || number.kind === "err")
      return [];
    return [
      {
        id: account.id,
        value: value.value,
        label: systemText(key, "ko"),
        number: number.value,
      },
    ];
  });

const AccountChoice = (props: {
  readonly selection: AccountSelection;
}): ReactElement | null => {
  const change = useAccountChange(useSystemClient());
  const choices = choicesOf(props.selection);
  const current = choices.find(
    (choice) => choice.id === props.selection.selected,
  );
  if (choices.length < 2 || current === undefined) return null;
  return (
    <Card title={systemText("system.accountTitle", "ko")}>
      <Stack gap="space-3">
        <Text>{systemText("system.accountBody", "ko")}</Text>
        <Select
          label={systemText("system.accountLabel", "ko")}
          options={choices.map((choice) => ({
            value: choice.value,
            label: choice.label,
          }))}
          value={current.value}
          disabled={change.state === "pending" ? "disabled" : "enabled"}
          onChange={(value): void => {
            const picked = choices.find((choice) => choice.value === value);
            if (picked !== undefined && picked.id !== current.id)
              change.select(picked.id);
          }}
        />
        <TextField
          label={systemText("system.accountNumber", "ko")}
          value={current.number}
          disabled="disabled"
        />
        {change.state === "failed"
          ? [
              <Text key="failed" tone="alert">
                {systemText("system.accountFailed", "ko")}
              </Text>,
            ]
          : []}
      </Stack>
    </Card>
  );
};

export const AccountCard = (): ReactElement | null => {
  const view = useAccounts(useSystemClient());
  switch (view.status) {
    case "loading":
    case "error":
      return null;
    case "ready":
      return <AccountChoice selection={view.selection} />;
  }
};
