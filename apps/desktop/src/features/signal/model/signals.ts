import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  toScreenState,
  type ScreenSources,
  type ScreenState,
  type SignalCardView,
  type SignalSource,
  type SignalsView,
} from "../../../shared/ipc";
import { queryKeys, screenKeys, type EntityId } from "../../../shared/lib";
import { detach } from "../../../shared/platform";

export type SourceNotice = "none" | "external-only";

export const countedSources = (
  sources: readonly SignalSource[],
): readonly SignalSource[] =>
  sources.flatMap((source): readonly SignalSource[] =>
    source.kind === "tool" ? [] : [source],
  );

export const sourceNotice = (
  sources: readonly SignalSource[],
): SourceNotice | "with-tool-data" => {
  const counted = countedSources(sources);
  if (counted.length === 0) return "none";
  return counted.length === sources.length ? "external-only" : "with-tool-data";
};

export type CardAction =
  | { readonly kind: "market-closed" }
  | { readonly kind: "no-order" }
  | { readonly kind: "zero-quantity" }
  | { readonly kind: "order"; readonly proposal: EntityId };

export const cardAction = (card: SignalCardView): CardAction => {
  if (card.market === "closed") return { kind: "market-closed" };
  if (card.sizing.kind === "zero") return { kind: "zero-quantity" };
  if (card.sizing.kind === "pending") return { kind: "no-order" };
  if (card.orderSwitch === "off" || card.proposal === undefined)
    return { kind: "no-order" };
  return { kind: "order", proposal: card.proposal };
};

export const useSignals = (
  load: ScreenSources["signals"],
): {
  readonly view: ScreenState<SignalsView>;
  readonly retry: () => void;
} => {
  const client = useQueryClient();
  const query = useQuery({ queryKey: screenKeys.signals, queryFn: load });
  return {
    view: toScreenState(query.data, (data) =>
      data.cards.length === 0 ? "empty" : "filled",
    ),
    retry: (): void => {
      detach({
        kind: "invalidate-queries",
        client,
        keys: [queryKeys.signals],
      });
    },
  };
};
