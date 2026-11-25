import { Channel } from "@tauri-apps/api/core";
import { err, ok, type EntityId, type ExternalUrl, type Result } from "../lib";
import { commands, events } from "./bindings";
import { settleBriefings } from "./briefing-schema";
import {
  parseChatId,
  parseChatStreamEvent,
  settleChatCancel,
  settleChatHistory,
  settleChatResubscribe,
  settleChatTurn,
} from "./chat-schema";
import type {
  ChatCancelOutcome,
  ChatHistory,
  ChatResubscribed,
  ChatStreamEvent,
  ChatTurnSettled,
  ChatTurnStart,
} from "./chat-types";
import type { BriefingsView } from "./estimate-views";
import {
  parseUiEvent,
  settleAppVersion,
  settleOpenExternal,
  wire,
  wireOutcome,
  type AppVersion,
  type CoreUiEvent,
  type IpcFailure,
  type OpenExternalResult,
} from "./schema";
import { settleUpdateStatus } from "./update-schema";
import type { UpdateRequest, UpdateStatus } from "./update-schema";
import {
  settleOrderSwitchOutcome,
  settleSettingsView,
  type OrderPermissionState,
  type OrderSwitchOutcome,
  type SettingsView,
} from "./settings-schema";
import { startIpcTimeout } from "./timer";

export type { IpcFailure } from "./schema";

export const frameworkFailure: IpcFailure = { kind: "framework" };
const timeoutFailure: IpcFailure = { kind: "timeout" };

const createTimeout = <T>(): {
  readonly promise: Promise<Result<T, IpcFailure>>;
  readonly cancel: () => void;
} => {
  const deferred = Promise.withResolvers<Result<T, IpcFailure>>();
  const cancel = startIpcTimeout((): void => {
    deferred.resolve(err(timeoutFailure));
  });
  return { promise: deferred.promise, cancel };
};

export const raceTimeout = <T>(
  pending: Promise<Result<T, IpcFailure>>,
): Promise<Result<T, IpcFailure>> => {
  const timer = createTimeout<T>();
  const settled = pending.then((result): Result<T, IpcFailure> => {
    timer.cancel();
    return result;
  });
  return Promise.race([settled, timer.promise]);
};

export const appVersion = (): Promise<Result<AppVersion, IpcFailure>> =>
  raceTimeout(
    commands
      .appVersion({ query: "current" })
      .then(settleAppVersion)
      .catch((): Result<AppVersion, IpcFailure> => err(frameworkFailure)),
  );

export const getSettings = (): Promise<Result<SettingsView, IpcFailure>> =>
  raceTimeout(
    commands
      .getSettings({ scope: "all" })
      .then(settleSettingsView)
      .catch((): Result<SettingsView, IpcFailure> => err(frameworkFailure)),
  );

export const openExternal = (
  url: ExternalUrl,
): Promise<Result<OpenExternalResult, IpcFailure>> =>
  raceTimeout(
    commands
      .openExternal({ url })
      .then(settleOpenExternal)
      .catch((): Result<OpenExternalResult, IpcFailure> =>
        err(frameworkFailure),
      ),
  );

export const getBriefings = (): Promise<Result<BriefingsView, IpcFailure>> =>
  raceTimeout(
    commands
      .getBriefings({ scope: "recent" })
      .then((outcome) => settleBriefings(wireOutcome(outcome)))
      .catch((): Result<BriefingsView, IpcFailure> => err(frameworkFailure)),
  );

export const setOrderPermission = (
  permission: OrderPermissionState,
): Promise<Result<OrderSwitchOutcome, IpcFailure>> =>
  raceTimeout(
    commands
      .setOrderPermission({ permission })
      .then(settleOrderSwitchOutcome)
      .catch((): Result<OrderSwitchOutcome, IpcFailure> =>
        err(frameworkFailure),
      ),
  );

export const listenUiEvent = (
  onEvent: (event: CoreUiEvent) => void,
): Promise<Result<() => void, IpcFailure>> =>
  events.uiEvent
    .listen((message): void => {
      const parsed = parseUiEvent(message.payload);
      if (parsed.kind === "ok") onEvent(parsed.value);
    })
    .then(ok, (): Result<() => void, IpcFailure> => err(frameworkFailure));

export type ChatListener = (event: Result<ChatStreamEvent, IpcFailure>) => void;

export type ChatTurnHandle = {
  readonly turn: EntityId;
  readonly settled: Promise<Result<ChatTurnSettled, IpcFailure>>;
};

const horizons = {
  "few-days": "FewDays",
  "weeks-to-months": "WeeksToMonths",
  "half-year": "HalfYearOrMore",
  hours: "Hours",
} as const;

const markets = { kr: "Kr", us: "Us" } as const;

const freshId = (
  prefix: "turn" | "run" | "message",
): Result<EntityId, IpcFailure> =>
  parseChatId(
    wire(
      `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 12)}`,
    ),
  );

export const mainChatSession = (): Result<EntityId, IpcFailure> =>
  parseChatId(wire("main-chat"));

export const newChatTurn = (): Result<EntityId, IpcFailure> => freshId("turn");

export const startChatTurn = (
  start: ChatTurnStart,
  listener: ChatListener,
): Result<ChatTurnHandle, IpcFailure> => {
  const run = freshId("run");
  const message = freshId("message");
  if (run.kind === "err" || message.kind === "err")
    return err(frameworkFailure);
  const settled = commands
    .sendChatMessage(
      {
        session_id: start.session,
        turn_id: start.turn,
        run_id: run.value,
        user_message_id: message.value,
        message: start.message,
        context:
          start.context === undefined
            ? null
            : {
                symbol: start.context.symbol ?? null,
                market:
                  start.context.market === undefined
                    ? null
                    : markets[start.context.market],
                horizon:
                  start.context.horizon === undefined
                    ? null
                    : horizons[start.context.horizon],
              },
        model: "Default",
        effort: "Default",
      },
      new Channel((raw): void => {
        listener(parseChatStreamEvent(wire(raw)));
      }),
    )
    .then((outcome) => settleChatTurn(wireOutcome(outcome)))
    .catch((): Result<ChatTurnSettled, IpcFailure> => err(frameworkFailure));
  return ok({ turn: start.turn, settled });
};

export const chatHistory = (
  session: EntityId,
): Promise<Result<ChatHistory, IpcFailure>> =>
  raceTimeout(
    commands
      .getChatHistory({ session_id: session })
      .then((outcome) => settleChatHistory(wireOutcome(outcome)))
      .catch((): Result<ChatHistory, IpcFailure> => err(frameworkFailure)),
  );

export const resubscribeChatTurn = (
  turn: EntityId,
  listener: ChatListener,
): Promise<Result<ChatResubscribed, IpcFailure>> =>
  commands
    .resubscribeChatTurn(
      { turn_id: turn, after_sequence: 0 },
      new Channel((raw): void => {
        listener(parseChatStreamEvent(wire(raw)));
      }),
    )
    .then((outcome) => settleChatResubscribe(wireOutcome(outcome)))
    .catch((): Result<ChatResubscribed, IpcFailure> => err(frameworkFailure));

export const cancelChatTurn = (
  turn: EntityId,
): Promise<Result<ChatCancelOutcome, IpcFailure>> =>
  raceTimeout(
    commands
      .cancelChatTurn({ turn_id: turn })
      .then((outcome) => settleChatCancel(wireOutcome(outcome)))
      .catch((): Result<ChatCancelOutcome, IpcFailure> =>
        err(frameworkFailure),
      ),
  );

export const updateCommand = (
  request: UpdateRequest,
): Promise<Result<UpdateStatus, IpcFailure>> =>
  raceTimeout(
    (request === "restart"
      ? commands.restartToUpdate({ confirm: request })
      : commands.getUpdateStatus({ query: request })
    )
      .then((outcome) => settleUpdateStatus(wireOutcome(outcome)))
      .catch((): Result<UpdateStatus, IpcFailure> => err(frameworkFailure)),
  );
