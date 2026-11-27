import type { QueryClient } from "@tanstack/react-query";
import { listenUiEvent, type CoreUiEvent } from "../ipc";
import { queryKeys, type AppQueryKey } from "../lib";

export type DetachedTask =
  | {
      readonly kind: "invalidate-queries";
      readonly client: Readonly<QueryClient>;
      readonly keys: readonly AppQueryKey[];
    }
  | {
      readonly kind: "invalidate-all" | "reset-chat";
      readonly client: Readonly<QueryClient>;
    };

const ignoreFailure = (): void => undefined;

export const detach = (task: DetachedTask): void => {
  switch (task.kind) {
    case "reset-chat": {
      task.client.removeQueries({ queryKey: queryKeys.chatResume });
      task.client
        .resetQueries({ queryKey: queryKeys.chatHistory })
        .then(ignoreFailure, ignoreFailure);
      return;
    }
    case "invalidate-all": {
      task.client.invalidateQueries().then(ignoreFailure, ignoreFailure);
      return;
    }
    case "invalidate-queries": {
      Promise.all(
        task.keys.map((queryKey) =>
          task.client.invalidateQueries({ queryKey }),
        ),
      ).then(ignoreFailure, ignoreFailure);
      return;
    }
  }
};

export const subscribeCoreEvents = (
  onEvent: (event: CoreUiEvent) => void,
): (() => void) => {
  const cancelled = Promise.withResolvers<"stopped">();
  const subscription = listenUiEvent(onEvent);
  Promise.all([subscription, cancelled.promise])
    .then(([result]): Promise<void> =>
      result.kind === "ok"
        ? Promise.resolve().then(result.value)
        : Promise.resolve(),
    )
    .catch(ignoreFailure);
  return (): void => {
    cancelled.resolve("stopped");
  };
};
