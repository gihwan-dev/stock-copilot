import { Outlet, useMatchRoute } from "@tanstack/react-router";
import { useState, type ReactElement } from "react";
import { InstrumentDetail, InstrumentList } from "../features/instruments";
import { SelfCheckBar } from "../features/self-check";
import { UpdateBar } from "../features/updates";
import { AppShell, IconButton, type PanelState } from "../shared/ds";
import { coreUpdateClient } from "../shared/ipc";
import { text, type EntityId } from "../shared/lib";
import { useNavigate, useSources } from "./context";

const destinations = [
  { path: "/", label: "nav.dashboard", icon: "chart-pie" },
  { path: "/briefing", label: "nav.briefing", icon: "newspaper" },
  { path: "/chat", label: "nav.chat", icon: "message-square" },
] as const;

export const ShellLayout = (): ReactElement => {
  const go = useNavigate();
  const sources = useSources();
  const [shown, setShown] = useState<EntityId | undefined>(undefined);
  const [listState, setListState] = useState<PanelState>("closed");
  const matchRoute = useMatchRoute();
  const list = <InstrumentList sources={sources} onSelect={setShown} />;
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
    shown === undefined ? (
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
        <>
          {destinations.map((item) => (
            <IconButton
              key={item.path}
              icon={item.icon}
              label={text(item.label, "ko")}
              tooltipSide="right"
              pressed={
                shown === undefined &&
                (matchRoute({ to: item.path }) !== false ||
                  (item.path === "/briefing" &&
                    matchRoute({ to: "/briefings" }) !== false))
                  ? "pressed"
                  : "released"
              }
              onClick={(): void => {
                setShown(undefined);
                setListState("closed");
                go(item.path);
              }}
            />
          ))}
        </>
      }
      main={
        shown === undefined &&
        (matchRoute({ to: "/" }) !== false ||
          matchRoute({ to: "/transactions" }) !== false)
          ? "fill"
          : "scroll"
      }
      listOpen={listState}
      onListOpenChange={setListState}
      list={list}
      status={status}
    >
      {page}
    </AppShell>
  );
};
