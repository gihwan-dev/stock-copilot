import { defaultParseSearch } from "@tanstack/react-router";
import type { Model, Action } from "flexlayout-react";
import { useEffect, useState } from "react";
import { QueryClient } from "@tanstack/react-query";
import { readWidgetLayout, listenUiEvent, type CoreUiEvent } from "../ipc";
import {
  defaultDock,
  restoreDock,
  queryKeys,
  emptyInputValue,
  parseInputValue,
  type InputValue,
  type AppQueryKey,
  type DockSnapshot,
  type DockProfile,
} from "../lib";

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

export const dockKey = (
  profile: DockProfile,
  version: "v1" | "v2",
): `stock-copilot.dashboard-dock.${"v1" | "v2"}.${DockProfile}` =>
  `stock-copilot.dashboard-dock.${version}.${profile}` as const;

const dockCache = new QueryClient({
  defaultOptions: { queries: { gcTime: Infinity } },
});
export const loadDockProfile = async (
  profile: DockProfile,
  read: () => DockSnapshot | undefined,
): Promise<DockSnapshot> => {
  const latest = (): DockSnapshot | undefined =>
    dockCache.getQueryData([profile]);
  const cached = latest();
  if (cached !== undefined) return cached;
  const saved = await Promise.resolve()
    .then(read)
    .catch((): undefined => undefined);
  return latest() ?? saved ?? defaultDock(profile);
};
export const saveDockProfile = (
  profile: DockProfile,
  model: DockSnapshot,
  write: () => void,
): void => {
  dockCache.setQueryData([profile], model);
  new Promise<void>((resolve) => {
    write();
    resolve();
  }).catch(ignoreFailure);
};

type WidgetGap = "0" | "4" | "8" | "12";
type WidgetPadding = "0" | "4" | "8" | "16";
type WidgetAppearance = {
  readonly gap: WidgetGap;
  readonly padding: WidgetPadding;
  readonly radius: WidgetGap;
};
type AppearanceField = "gap" | "padding" | "radius";
type AppearanceStorage = {
  readonly read: (key: InputValue) => InputValue | undefined;
  readonly write: (key: InputValue, value: InputValue) => void;
};
type AppearanceSettings = {
  readonly prefs: WidgetAppearance;
  readonly change: (field: AppearanceField, value: InputValue) => void;
};

const defaults: WidgetAppearance = { gap: "0", padding: "8", radius: "0" };
const appearanceKey = (field: AppearanceField): InputValue => {
  const parsed = parseInputValue(`stock-copilot.appearance.${field}`);
  return parsed.kind === "ok" ? parsed.value : emptyInputValue;
};

const gapValue = (value: InputValue | undefined): WidgetGap =>
  (["4", "8", "12"] as const).find((option) => option === value) ?? "0";
const paddingValue = (value: InputValue | undefined): WidgetPadding =>
  (["0", "4", "16"] as const).find((option) => option === value) ?? "8";

const loadAppearance = (
  read: AppearanceStorage["read"],
): Promise<WidgetAppearance> =>
  Promise.resolve()
    .then(() => ({
      gap: gapValue(read(appearanceKey("gap"))),
      padding: paddingValue(read(appearanceKey("padding"))),
      radius: gapValue(read(appearanceKey("radius"))),
    }))
    .then(
      (loaded) => loaded,
      () => defaults,
    );

export const useAppearanceState = ({
  read,
  write,
}: AppearanceStorage): AppearanceSettings => {
  const [saved, setSaved] = useState(defaults);
  const [changes, setChanges] = useState<Partial<WidgetAppearance>>({});
  useEffect((): void => {
    loadAppearance(read).then(setSaved, ignoreFailure);
  }, [read]);
  return {
    prefs: { ...saved, ...changes },
    change: (field, value): void => {
      setChanges((previous) =>
        field === "padding"
          ? { ...previous, padding: paddingValue(value) }
          : { ...previous, [field]: gapValue(value) },
      );
      Promise.resolve()
        .then((): void => {
          write(appearanceKey(field), value);
        })
        .then(ignoreFailure, ignoreFailure);
    },
  };
};

export const useInitialDock = (
  profile: DockProfile,
  read: (profile: DockProfile) => Promise<DockSnapshot>,
): DockSnapshot | undefined => {
  const [snapshot, setSnapshot] = useState<{
    readonly profile: DockProfile;
    readonly initial: DockSnapshot;
  }>();
  useEffect(() => {
    const unmounted = Promise.withResolvers<undefined>();
    Promise.race([read(profile), unmounted.promise]).then((initial): void => {
      if (initial !== undefined) setSnapshot({ profile, initial });
    }, ignoreFailure);
    return (): void => {
      unmounted.resolve(undefined);
    };
  }, [profile, read]);
  return snapshot?.profile === profile ? snapshot.initial : undefined;
};

export const useDockPersistence = (
  model: Model | undefined,
  save: () => void,
): void => {
  useEffect(() => {
    if (model === undefined) return;
    const changed = (action: Readonly<Action>): void => {
      if (!action.isAdjusting()) save();
    };
    model.addChangeListener(changed);
    return (): void => {
      model.removeChangeListener(changed);
    };
  }, [model, save]);
};

export const applyDockAction = (
  model: Readonly<Model>,
  action: Readonly<Action>,
): void => {
  model.doAction(action);
};

export const decodeDockLayout = (
  profile: DockProfile,
  read: (key: ReturnType<typeof dockKey>) => InputValue | undefined,
): DockSnapshot | undefined => {
  const native = read(dockKey(profile, "v2"));
  if (native !== undefined && native.length <= 65536) {
    const restored = readWidgetLayout(defaultParseSearch(native));
    if (restored !== undefined) return { version: 2, layout: restored };
  }
  const legacy = read(dockKey(profile, "v1"));
  return legacy === undefined || legacy.length > 65536
    ? undefined
    : restoreDock(defaultParseSearch(legacy));
};
