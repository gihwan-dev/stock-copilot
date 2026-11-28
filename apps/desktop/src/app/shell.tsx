import { Outlet, useMatchRoute } from "@tanstack/react-router";
import { useState, type ReactElement } from "react";
import { InstrumentDetail, InstrumentList } from "../features/instruments";
import { SelfCheckBar } from "../features/self-check";
import { UpdateBar } from "../features/updates";
import { AppShell, IconButton, type PanelState } from "../shared/ds";
import { coreUpdateClient } from "../shared/ipc";
import { text, type EntityId } from "../shared/lib";
import { useNavigate, useSources, type NavigateFn } from "./context";

const destinations = [
  { path: "/", label: "nav.dashboard", icon: "chart-pie" },
  { path: "/briefing", label: "nav.briefing", icon: "newspaper" },
  { path: "/chat", label: "nav.chat", icon: "message-square" },
  { path: "/settings", label: "app.openSettings", icon: "settings" },
] as const;

const Destinations = (props: {
  readonly shown: EntityId | undefined;
  readonly open: NavigateFn;
}): ReactElement => {
  const matchRoute = useMatchRoute();
  return (
    <>
      {destinations.map((item) => (
        <IconButton
          key={item.path}
          icon={item.icon}
          label={text(item.label, "ko")}
          caption="wide"
          tooltipSide="right"
          pressed={
            props.shown === undefined &&
            (matchRoute({ to: item.path }) !== false ||
              (item.path === "/briefing" &&
                matchRoute({ to: "/briefings" }) !== false) ||
              (item.path === "/settings" &&
                matchRoute({ to: "/settings/status" }) !== false))
              ? "pressed"
              : "released"
          }
          onClick={(): void => {
            props.open(item.path);
          }}
        />
      ))}
    </>
  );
};

export const ShellLayout = (): ReactElement => {
  const go = useNavigate();
  const sources = useSources();
  const [shown, setShown] = useState<EntityId | undefined>(undefined);
  const [listState, setListState] = useState<PanelState>("closed");
  const matchRoute = useMatchRoute();
  const settings = matchRoute({ to: "/settings", fuzzy: true }) !== false;
  const status = (
    <>
      <UpdateBar client={coreUpdateClient} />
      <SelfCheckBar
        load={sources.selfCheck}
        onOpenSettings={(): void => {
          setShown(undefined);
          go("/settings");
        }}
      />
    </>
  );
  const page =
    shown === undefined || settings ? (
      <Outlet />
    ) : (
      <InstrumentDetail
        key={shown}
        sources={sources}
        instrument={shown}
        onClose={(): void => {
          setShown(undefined);
        }}
      />
    );
  return (
    <AppShell
      navigation={
        <Destinations
          shown={settings ? undefined : shown}
          open={(path): void => {
            setShown(undefined);
            setListState("closed");
            go(path);
          }}
        />
      }
      main={
        shown === undefined &&
        (matchRoute({ to: "/chat" }) !== false ||
          matchRoute({ to: "/" }) !== false ||
          matchRoute({ to: "/transactions" }) !== false)
          ? "fill"
          : "scroll"
      }
      listOpen={listState}
      onListOpenChange={setListState}
      list={
        settings ? null : (
          <InstrumentList sources={sources} onSelect={setShown} />
        )
      }
      status={settings ? null : status}
    >
      {page}
    </AppShell>
  );
};
