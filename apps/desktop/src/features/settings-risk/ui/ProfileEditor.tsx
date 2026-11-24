import { useState, type ReactElement } from "react";
import {
  Button,
  Card,
  Checkbox,
  EmptyState,
  Select,
  Skeleton,
  Stack,
  Text,
} from "../../../shared/ds";
import {
  defaultProfile,
  excludableTypes,
  instrumentCaps,
  profileHorizons,
  riskAppetites,
  sectorCaps,
  type ExcludableType,
  type ProfileDraft,
  type ScreenSources,
} from "../../../shared/ipc";
import { riskText } from "../messages";
import {
  choicesOf,
  inputOf,
  pick,
  useDraftSource,
  useSave,
} from "../model/risk";
import { SaveNotice } from "./SaveNotice";

type Sources = Pick<ScreenSources, "investorProfile" | "saveInvestorProfile">;
type Edit = {
  readonly draft: ProfileDraft;
  readonly change: (draft: ProfileDraft) => void;
};

const horizonChoices = choicesOf(profileHorizons, (value) =>
  riskText(`risk.horizon.${value}`, "ko"),
);
const appetiteChoices = choicesOf(riskAppetites, (value) =>
  riskText(`risk.appetite.${value}`, "ko"),
);
const instrumentCapChoices = choicesOf(instrumentCaps, (value) =>
  riskText(`risk.cap.${value}`, "ko"),
);
const sectorCapChoices = choicesOf(sectorCaps, (value) =>
  riskText(`risk.cap.${value}`, "ko"),
);

const toggleType = (
  draft: ProfileDraft,
  type: ExcludableType,
  state: "checked" | "unchecked",
): ProfileDraft => {
  const others = draft.excludedTypes.filter((item) => item !== type);
  return {
    ...draft,
    excludedTypes: state === "checked" ? [...others, type] : others,
  };
};

const StyleFields = (props: Edit): ReactElement => (
  <Stack gap="space-4">
    <Select
      label={riskText("risk.horizon", "ko")}
      options={horizonChoices}
      value={inputOf(props.draft.horizon)}
      onChange={(value): void => {
        const horizon = pick(profileHorizons, value);
        if (horizon !== undefined) props.change({ ...props.draft, horizon });
      }}
    />
    <Select
      label={riskText("risk.appetite", "ko")}
      options={appetiteChoices}
      value={inputOf(props.draft.riskAppetite)}
      onChange={(value): void => {
        const riskAppetite = pick(riskAppetites, value);
        if (riskAppetite !== undefined)
          props.change({ ...props.draft, riskAppetite });
      }}
    />
  </Stack>
);

const CapFields = (props: Edit): ReactElement => (
  <Stack gap="space-4">
    <Select
      label={riskText("risk.instrumentCap", "ko")}
      options={instrumentCapChoices}
      value={inputOf(props.draft.instrumentCap)}
      onChange={(value): void => {
        const instrumentCap = pick(instrumentCaps, value);
        if (instrumentCap !== undefined)
          props.change({ ...props.draft, instrumentCap });
      }}
    />
    <Select
      label={riskText("risk.sectorCap", "ko")}
      options={sectorCapChoices}
      value={inputOf(props.draft.sectorCap)}
      onChange={(value): void => {
        const sectorCap = pick(sectorCaps, value);
        if (sectorCap !== undefined)
          props.change({ ...props.draft, sectorCap });
      }}
    />
  </Stack>
);

const ExclusionFields = (props: Edit): ReactElement => (
  <Stack gap="space-2">
    <Text variant="label">{riskText("risk.excluded", "ko")}</Text>
    {excludableTypes.map((type): ReactElement => (
      <Checkbox
        key={type}
        label={riskText(`risk.type.${type}`, "ko")}
        checked={
          props.draft.excludedTypes.includes(type) ? "checked" : "unchecked"
        }
        onChange={(state): void => {
          props.change(toggleType(props.draft, type, state));
        }}
      />
    ))}
  </Stack>
);

const Form = (props: {
  readonly initial: ProfileDraft;
  readonly sources: Sources;
  readonly origin: "stored" | "defaults";
}): ReactElement => {
  const [draft, setDraft] = useState<ProfileDraft>(props.initial);
  const save = useSave(props.sources.saveInvestorProfile);
  return (
    <Stack gap="space-4">
      {props.origin === "defaults" ? (
        <Text variant="caption" tone="ink-muted">
          {riskText("risk.defaults", "ko")}
        </Text>
      ) : (
        <Stack gap="space-1">{[]}</Stack>
      )}
      <StyleFields draft={draft} change={setDraft} />
      <CapFields draft={draft} change={setDraft} />
      <ExclusionFields draft={draft} change={setDraft} />
      <Text variant="caption" tone="ink-muted">
        {riskText("risk.confirmNote", "ko")}
      </Text>
      <Button
        variant="primary"
        onClick={(): void => {
          save.submit(draft);
        }}
      >
        {riskText("risk.save", "ko")}
      </Button>
      <SaveNotice notice={save.notice} outcome={save.reason} />
    </Stack>
  );
};

const Body = (props: { readonly sources: Sources }): ReactElement => {
  const source = useDraftSource(
    ["settings", "risk", "profile"],
    props.sources.investorProfile,
  );
  switch (source.view.status) {
    case "loading":
      return <Skeleton variant="text" lines={3} />;
    case "error":
      return (
        <EmptyState
          title={riskText("risk.loadFailed", "ko")}
          action={{
            label: riskText("risk.retry", "ko"),
            onPress: source.retry,
          }}
        />
      );
    case "empty":
      return (
        <Form
          initial={defaultProfile}
          sources={props.sources}
          origin="defaults"
        />
      );
    case "ready":
      return (
        <Form
          initial={source.view.data}
          sources={props.sources}
          origin="stored"
        />
      );
  }
};

export const ProfileEditor = (props: {
  readonly sources: Sources;
}): ReactElement => (
  <Card title={riskText("risk.profileTitle", "ko")}>
    <Body sources={props.sources} />
  </Card>
);
