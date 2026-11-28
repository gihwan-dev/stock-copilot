import { useEffect, useLayoutEffect, useState } from "react";
import { QueryClient } from "@tanstack/react-query";
import { listenUiEvent, type CoreUiEvent } from "../ipc";
import {
  defaultDock,
  preferredDockAfterEdit,
  visibleDockModel,
  useDockPreferences,
  type DockArea,
  queryKeys,
  emptyInputValue,
  parseInputValue,
  type InputValue,
  type AppQueryKey,
  type DockModel,
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
): `stock-copilot.dashboard-dock.v1.${DockProfile}` =>
  `stock-copilot.dashboard-dock.v1.${profile}` as const;

const dockCache = new QueryClient({
  defaultOptions: { queries: { gcTime: Infinity } },
});
export const loadDockProfile = async (
  profile: DockProfile,
  read: () => DockModel | undefined,
): Promise<DockModel> => {
  const latest = (): DockModel | undefined => dockCache.getQueryData([profile]);
  const cached = latest();
  if (cached !== undefined) return cached;
  const saved = await Promise.resolve()
    .then(read)
    .catch((): undefined => undefined);
  return latest() ?? saved ?? defaultDock(profile);
};
export const saveDockProfile = (
  profile: DockProfile,
  model: DockModel,
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
  read: (profile: DockProfile) => Promise<DockModel>,
): DockModel | undefined => {
  const [snapshot, setSnapshot] = useState<{
    readonly profile: DockProfile;
    readonly initial: DockModel;
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

type DockSession = ReturnType<typeof useDockPreferences>;
type DockOptions = Parameters<typeof useDockPreferences>[0] & {
  readonly watch: (change: (area: DockArea) => void) => () => void;
};

const showDockWithin = (
  handle: DockSession["handle"],
  desired: DockModel,
  area: DockArea | undefined,
): void => {
  if (handle === null) return;
  const next = visibleDockModel(handle.getModel(), desired, area);
  if (next !== undefined) handle.setModel(next);
};

export const useFittedDock = (options: DockOptions): DockSession => {
  const [area, setArea] = useState<DockArea>();
  const { profile, initial, watch } = options;
  const session = useDockPreferences({
    ...options,
    persist(key, next, handle): void {
      const original = dockCache.getQueryData<DockModel>([key]) ?? initial;
      const desired = preferredDockAfterEdit(next, original, area);
      options.persist(key, desired, handle);
      showDockWithin(handle, desired, area);
    },
  });
  const { handle } = session;
  useLayoutEffect(() => {
    if (handle !== null) return watch(setArea);
    return undefined;
  }, [handle, watch]);
  useLayoutEffect(() => {
    const desired = dockCache.getQueryData<DockModel>([profile]) ?? initial;
    if (desired !== undefined) showDockWithin(handle, desired, area);
  }, [area, handle, initial, profile]);
  return session;
};
