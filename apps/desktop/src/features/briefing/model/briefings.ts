import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  openExternal,
  toScreenState,
  type BriefingSummary,
  type BriefingsView,
  type ScreenSources,
  type ScreenState,
} from "../../../shared/ipc";
import { queryKeys, screenKeys, type ExternalUrl } from "../../../shared/lib";
import { detach } from "../../../shared/platform";

export type LinkVerdict = "allow" | "deny";

export const linkVerdict = (
  briefing: BriefingSummary,
  url: ExternalUrl,
): LinkVerdict => {
  if (briefing.outcome.kind === "failed") return "deny";
  return briefing.outcome.sources.some((source) => source.url === url)
    ? "allow"
    : "deny";
};

type BriefingScope = "latest" | "past";

const fill = (
  view: BriefingsView,
  scope: BriefingScope,
): "filled" | "empty" => {
  if (scope === "latest") return view.latest === undefined ? "empty" : "filled";
  return view.past.length === 0 ? "empty" : "filled";
};

export const useBriefings = (
  load: ScreenSources["briefings"],
  scope: BriefingScope,
): {
  readonly view: ScreenState<BriefingsView>;
  readonly retry: () => void;
} => {
  const client = useQueryClient();
  const query = useQuery({ queryKey: screenKeys.briefings, queryFn: load });
  return {
    view: toScreenState(query.data, (data) => fill(data, scope)),
    retry: (): void => {
      detach({
        kind: "invalidate-queries",
        client,
        keys: [queryKeys.briefings],
      });
    },
  };
};

export const useOpenSource = (): ((url: ExternalUrl) => void) => {
  const mutation = useMutation({ mutationFn: openExternal });
  return (url): void => {
    mutation.mutate(url);
  };
};
