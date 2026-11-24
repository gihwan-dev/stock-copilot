import { useState, type ReactElement } from "react";
import {
  Button,
  Card,
  Checkbox,
  Select,
  Skeleton,
  Stack,
  Text,
} from "../../../shared/ds";
import {
  excludedKinds,
  horizons,
  riskLevels,
  sectorCapPercents,
  stockCapPercents,
  type ExcludedKind,
  type Horizon,
  type InvestmentProfile,
  type RiskLevel,
  type SectorCapPercent,
  type StockCapPercent,
} from "../../../shared/ipc";
import {
  emptyInputValue,
  parseInputValue,
  type InputValue,
  type UiText,
} from "../../../shared/lib";
import { basicText } from "../messages";
import { useProfile, useProfileSaver, type SaveState } from "../model";
import { useBasicClient } from "./client";

type Choice = { readonly value: InputValue; readonly label: UiText };
type Literal = Horizon | RiskLevel | StockCapPercent | SectorCapPercent;

const inputOf = (raw: Literal): InputValue => {
  const value = parseInputValue(raw);
  return value.kind === "ok" ? value.value : emptyInputValue;
};

const choices = <T extends Literal>(
  values: readonly T[],
  label: (value: T) => UiText,
): readonly Choice[] =>
  values.flatMap((raw) => {
    const value = parseInputValue(raw);
    return value.kind === "ok"
      ? [{ value: value.value, label: label(raw) }]
      : [];
  });

const horizonChoices = choices(horizons, (value) =>
  basicText(`basic.horizon.${value}`, "ko"),
);
const riskChoices = choices(riskLevels, (value) =>
  basicText(`basic.risk.${value}`, "ko"),
);
const stockCapChoices = choices(stockCapPercents, (value) =>
  basicText(`basic.stockCap.${value}`, "ko"),
);
const sectorCapChoices = choices(sectorCapPercents, (value) =>
  basicText(`basic.sectorCap.${value}`, "ko"),
);

const ChoiceField = <T extends Literal>(props: {
  readonly label: UiText;
  readonly options: readonly Choice[];
  readonly values: readonly T[];
  readonly value: T;
  readonly onChange: (value: T) => void;
}): ReactElement => (
  <Select
    label={props.label}
    options={props.options}
    value={inputOf(props.value)}
    onChange={(next): void => {
      props.values
        .filter((item) => inputOf(item) === next)
        .forEach((picked): void => {
          props.onChange(picked);
        });
    }}
  />
);

const toggled = (
  list: readonly ExcludedKind[],
  kind: ExcludedKind,
  state: "checked" | "unchecked",
): readonly ExcludedKind[] => {
  const rest = list.filter((item) => item !== kind);
  return state === "checked"
    ? excludedKinds.filter((item) => item === kind || rest.includes(item))
    : rest;
};

const Exclusions = (props: {
  readonly value: readonly ExcludedKind[];
  readonly onChange: (value: readonly ExcludedKind[]) => void;
}): ReactElement => (
  <Stack gap="space-2">
    <Text variant="label">{basicText("basic.excludeLabel", "ko")}</Text>
    {excludedKinds.map((kind) => (
      <Checkbox
        key={kind}
        label={basicText(`basic.exclude.${kind}`, "ko")}
        checked={props.value.includes(kind) ? "checked" : "unchecked"}
        onChange={(state): void => {
          props.onChange(toggled(props.value, kind, state));
        }}
      />
    ))}
  </Stack>
);

const saveLine = (state: SaveState): readonly ReactElement[] => {
  switch (state) {
    case "idle":
    case "pending":
      return [];
    case "saved":
      return [
        <Text key="saved" tone="ink-muted">
          {basicText("basic.profileSaved", "ko")}
        </Text>,
      ];
    case "failed":
      return [
        <Text key="failed" tone="alert">
          {basicText("basic.saveFailed", "ko")}
        </Text>,
      ];
  }
};

type Patch = (next: Partial<InvestmentProfile>) => void;

const ProfileFields = (props: {
  readonly draft: InvestmentProfile;
  readonly patch: Patch;
}): ReactElement => {
  const draft = props.draft;
  const patch = props.patch;
  return (
    <Stack gap="space-3">
      <ChoiceField
        label={basicText("basic.horizonLabel", "ko")}
        options={horizonChoices}
        values={horizons}
        value={draft.horizon}
        onChange={(horizon): void => {
          patch({ horizon });
        }}
      />
      <ChoiceField
        label={basicText("basic.riskLabel", "ko")}
        options={riskChoices}
        values={riskLevels}
        value={draft.risk}
        onChange={(risk): void => {
          patch({ risk });
        }}
      />
      <Exclusions
        value={draft.excluded_kinds}
        onChange={(excluded_kinds): void => {
          patch({ excluded_kinds });
        }}
      />
      <ChoiceField
        label={basicText("basic.stockCapLabel", "ko")}
        options={stockCapChoices}
        values={stockCapPercents}
        value={draft.stock_cap_percent}
        onChange={(stock_cap_percent): void => {
          patch({ stock_cap_percent });
        }}
      />
      <ChoiceField
        label={basicText("basic.sectorCapLabel", "ko")}
        options={sectorCapChoices}
        values={sectorCapPercents}
        value={draft.sector_cap_percent}
        onChange={(sector_cap_percent): void => {
          patch({ sector_cap_percent });
        }}
      />
    </Stack>
  );
};

const ProfileForm = (props: {
  readonly profile: InvestmentProfile;
  readonly state: SaveState;
  readonly save: (profile: InvestmentProfile) => void;
}): ReactElement => {
  const [draft, setDraft] = useState<InvestmentProfile>(props.profile);
  const patch = (next: Partial<InvestmentProfile>): void => {
    setDraft((current) => ({ ...current, ...next }));
  };
  return (
    <Stack gap="space-3">
      <ProfileFields draft={draft} patch={patch} />
      <Button
        variant="primary"
        disabled={props.state === "pending" ? "disabled" : "enabled"}
        onClick={(): void => {
          props.save(draft);
        }}
      >
        {basicText("basic.profileSave", "ko")}
      </Button>
      {saveLine(props.state)}
    </Stack>
  );
};

const Body = (): ReactElement => {
  const client = useBasicClient();
  const view = useProfile(client);
  const saver = useProfileSaver(client);
  switch (view.status) {
    case "loading":
      return <Skeleton variant="text" />;
    case "error":
      return <Text tone="alert">{basicText("basic.profileFailed", "ko")}</Text>;
    case "ready":
      return (
        <ProfileForm
          profile={view.data}
          state={saver.state}
          save={saver.save}
        />
      );
  }
};

export const ProfileCard = (): ReactElement => (
  <Card title={basicText("basic.profileTitle", "ko")}>
    <Body />
  </Card>
);
