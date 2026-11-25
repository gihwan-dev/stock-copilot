import type { EntityId, Result, Ticker } from "../lib";
import { commands } from "./bindings";
import { settleCall } from "./call";
import type {
  InstrumentDetailView,
  InstrumentListView,
  WatchlistChange,
} from "./estimate-views";
import {
  settleInstrumentDetail,
  settleInstruments,
  settleWatchlistChange,
} from "./instruments-schema";
import { wireOutcome, type IpcFailure } from "./schema";

type Reply<T> = Promise<Result<T, IpcFailure>>;

export const getInstruments = (): Reply<InstrumentListView> =>
  settleCall(commands.getInstruments({ scope: "tracked" }), (outcome) =>
    settleInstruments(wireOutcome(outcome)),
  );

export const getInstrumentDetail = (
  instrument: EntityId,
): Reply<InstrumentDetailView> =>
  settleCall(commands.getInstrumentDetail({ symbol: instrument }), (outcome) =>
    settleInstrumentDetail(wireOutcome(outcome)),
  );

export const addWatchlist = (ticker: Ticker): Reply<WatchlistChange> =>
  settleCall(commands.addWatchlist({ symbol: ticker }), (outcome) =>
    settleWatchlistChange(wireOutcome(outcome)),
  );

export const removeWatchlist = (instrument: EntityId): Reply<WatchlistChange> =>
  settleCall(commands.removeWatchlist({ symbol: instrument }), (outcome) =>
    settleWatchlistChange(wireOutcome(outcome)),
  );
