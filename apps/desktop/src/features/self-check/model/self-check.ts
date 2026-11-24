import { useQuery } from "@tanstack/react-query";
import type {
  IpcFailure,
  ScreenSources,
  SelfCheckProblem,
  SelfCheckView,
} from "../../../shared/ipc";
import { screenKeys, type Result } from "../../../shared/lib";

type SelfCheckAction = "open-settings" | "none";

export type SelfCheckNotice =
  | { readonly kind: "hidden" }
  | {
      readonly kind: "shown";
      readonly problem: SelfCheckProblem;
      readonly action: SelfCheckAction;
    };

const actions = {
  "toss-key-invalid": "open-settings",
  "toss-key-permission": "open-settings",
  "toss-account-missing": "open-settings",
  "ip-not-allowed": "open-settings",
  "toss-key-shared": "open-settings",
  "claude-missing": "none",
  "claude-signed-out": "none",
  "order-risk-notice": "none",
  "order-exchange-setting": "none",
  "order-dollar-shortage": "none",
} as const satisfies Record<SelfCheckProblem, SelfCheckAction>;

const noticeFor = (
  result: Result<SelfCheckView, IpcFailure> | undefined,
): SelfCheckNotice => {
  if (result?.kind !== "ok" || result.value.kind === "clear")
    return { kind: "hidden" };
  return {
    kind: "shown",
    problem: result.value.problem,
    action: actions[result.value.problem],
  };
};

export const useSelfCheckNotice = (
  load: ScreenSources["selfCheck"],
): SelfCheckNotice => {
  const query = useQuery({ queryKey: screenKeys.selfCheck, queryFn: load });
  return noticeFor(query.data);
};
