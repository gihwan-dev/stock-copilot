import { createContext, use, type ReactElement } from "react";

export type KeyEntry =
  | { readonly kind: "absent" }
  | { readonly kind: "present"; readonly open: () => void };

const absent: KeyEntry = { kind: "absent" };

const EntryContext = createContext<KeyEntry>(absent);

export const KeyEntryProvider = (props: {
  readonly entry: KeyEntry;
  readonly children: ReactElement;
}): ReactElement => (
  <EntryContext value={props.entry}>{props.children}</EntryContext>
);

export const useKeyEntry = (): KeyEntry => use(EntryContext);
