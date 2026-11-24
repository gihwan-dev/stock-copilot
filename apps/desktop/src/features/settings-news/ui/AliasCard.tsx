import { useId, useState, type ReactElement } from "react";
import {
  Button,
  Card,
  Inline,
  Stack,
  Text,
  TextField,
} from "../../../shared/ds";
import {
  parseAlias,
  parseTickerField,
  type AliasRow,
} from "../../../shared/ipc";
import {
  emptyInputValue,
  parseInputValue,
  type InputValue,
} from "../../../shared/lib";
import { newsText } from "../messages";
import type { NewsChange } from "../model";

type Draft = {
  readonly key: InputValue;
  readonly alias: InputValue;
  readonly ticker: InputValue;
};

const keyOf = (base: InputValue, position: InputValue): InputValue => {
  const key = parseInputValue(`${base}-${position}`);
  return key.kind === "ok" ? key.value : base;
};

const newKey = (last: InputValue): InputValue => {
  const key = parseInputValue(`${last}n`);
  return key.kind === "ok" ? key.value : last;
};

const draftsOf = (
  base: InputValue,
  rows: readonly AliasRow[],
): readonly Draft[] =>
  rows.flatMap((row, index) => {
    const alias = parseInputValue(row.alias);
    const ticker = parseInputValue(row.ticker);
    const position = parseInputValue("r".repeat(index + 1));
    if (
      alias.kind === "err" ||
      ticker.kind === "err" ||
      position.kind === "err"
    )
      return [];
    return [
      {
        key: keyOf(base, position.value),
        alias: alias.value,
        ticker: ticker.value,
      },
    ];
  });

const rowsOf = (drafts: readonly Draft[]): readonly AliasRow[] | undefined => {
  const rows = drafts.flatMap((draft) => {
    const alias = parseAlias(draft.alias);
    const ticker = parseTickerField(draft.ticker);
    return alias.kind === "ok" && ticker.kind === "ok"
      ? [{ alias: alias.value, ticker: ticker.value }]
      : [];
  });
  return rows.length === drafts.length ? rows : undefined;
};

const DraftRow = (props: {
  readonly draft: Draft;
  readonly change: (draft: Draft) => void;
  readonly remove: () => void;
}): ReactElement => (
  <Inline gap="space-2" align="end" wrap="wrap">
    <TextField
      label={newsText("news.alias", "ko")}
      value={props.draft.alias}
      onChange={(alias): void => {
        props.change({ ...props.draft, alias });
      }}
    />
    <TextField
      label={newsText("news.ticker", "ko")}
      value={props.draft.ticker}
      onChange={(ticker): void => {
        props.change({ ...props.draft, ticker });
      }}
    />
    <Button variant="ghost" onClick={props.remove}>
      {newsText("news.aliasRemove", "ko")}
    </Button>
  </Inline>
);

type AliasDrafts = {
  readonly drafts: readonly Draft[];
  readonly invalid: "valid" | "invalid";
  readonly add: () => void;
  readonly change: (draft: Draft) => void;
  readonly remove: (key: InputValue) => void;
  readonly submit: () => void;
};

const useAliasDrafts = (
  aliases: readonly AliasRow[],
  save: (change: NewsChange) => void,
): AliasDrafts => {
  const baseId = parseInputValue(useId());
  const base = baseId.kind === "ok" ? baseId.value : emptyInputValue;
  const [drafts, setDrafts] = useState(() => draftsOf(base, aliases));
  const [last, setLast] = useState<InputValue>(base);
  const [invalid, setInvalid] = useState<"valid" | "invalid">("valid");
  return {
    drafts,
    invalid,
    add: (): void => {
      const key = newKey(last);
      setLast(key);
      setDrafts([
        ...drafts,
        { key, alias: emptyInputValue, ticker: emptyInputValue },
      ]);
    },
    change: (next): void => {
      setDrafts(drafts.map((row) => (row.key === next.key ? next : row)));
    },
    remove: (key): void => {
      setDrafts(drafts.filter((row) => row.key !== key));
    },
    submit: (): void => {
      const rows = rowsOf(drafts);
      setInvalid(rows === undefined ? "invalid" : "valid");
      if (rows !== undefined) save({ kind: "aliases", aliases: rows });
    },
  };
};

export const AliasCard = (props: {
  readonly aliases: readonly AliasRow[];
  readonly save: (change: NewsChange) => void;
}): ReactElement => {
  const form = useAliasDrafts(props.aliases, props.save);
  return (
    <Card title={newsText("news.aliasTitle", "ko")}>
      <Stack gap="space-3">
        <Text tone="ink-muted">{newsText("news.aliasBody", "ko")}</Text>
        {form.drafts.map((draft) => (
          <DraftRow
            key={draft.key}
            draft={draft}
            change={form.change}
            remove={(): void => {
              form.remove(draft.key);
            }}
          />
        ))}
        {form.invalid === "invalid"
          ? [
              <Text key="invalid" tone="alert">
                {newsText("news.aliasInvalid", "ko")}
              </Text>,
            ]
          : []}
        <Inline gap="space-2" wrap="wrap">
          <Button variant="secondary" onClick={form.add}>
            {newsText("news.aliasAdd", "ko")}
          </Button>
          <Button variant="primary" onClick={form.submit}>
            {newsText("news.save", "ko")}
          </Button>
        </Inline>
      </Stack>
    </Card>
  );
};
