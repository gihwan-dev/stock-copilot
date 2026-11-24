import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { vi, type VitestUtils } from "vitest";
import type { ExternalUrl } from "../../lib";
import { mockCommands } from "./mockBindings";
import { responses } from "./responses";

const frameworkRejection = "app_version not allowed. Plugin not found";

export const installCommandWindow = (): VitestUtils =>
  vi.stubGlobal("window", {});

export const installChannelWindow = (): VitestUtils =>
  vi.stubGlobal("window", {
    crypto: {
      getRandomValues: <T>(values: Readonly<T>): Readonly<T> => values,
    },
  });

export const answerWithRecordedResponses = (): void => {
  mockCommands({ appVersion: () => responses.appVersion });
};

export const rejectWithFrameworkError = (): void => {
  mockCommands({
    appVersion: () => {
      throw frameworkRejection;
    },
  });
};

export const returnRecordedCommandError = (): void => {
  mockCommands({
    appVersion: () => {
      throw responses.ipcErrors[2];
    },
  });
};

export const answerWithMalformedResponse = (): void => {
  mockCommands({ appVersion: () => ({ version: "not a version" }) });
};

export const rejectWithErrorObject = (): void => {
  mockCommands({
    appVersion: () => {
      throw new Error("invoke failed");
    },
  });
};

export const rejectWithUnknownObject = (): void => {
  mockCommands({
    appVersion: () => {
      throw { reason: "denied" };
    },
  });
};

export const rejectWithUndefined = (): void => {
  mockCommands({
    appVersion: () => {
      throw undefined;
    },
  });
};

export const hangAppVersion = (): void => {
  mockCommands({
    appVersion: () => new Promise(() => undefined),
  });
};

export const enableFakeCommandTimers = (): void => {
  const timers = vi.useFakeTimers();
  return timers === timers ? undefined : undefined;
};

export const advanceCommandTimers = (): Promise<void> =>
  vi.advanceTimersByTimeAsync(10_000).then((): void => undefined);

export const resetCommands = (): VitestUtils => {
  clearMocks();
  return vi.useRealTimers().unstubAllGlobals();
};

export const answerOpenExternalForExpectedUrl = (
  expectedUrl: ExternalUrl,
): void => {
  mockCommands({
    openExternal: ({ request }) => {
      if (request.url !== expectedUrl) {
        throw new Error(`unexpected url: ${request.url}`);
      }
      return responses.openExternal;
    },
  });
};

export const returnRecordedOpenExternalError = (): void => {
  mockCommands({
    openExternal: () => {
      throw responses.ipcErrors[2];
    },
  });
};

export const answerOpenExternalWithMalformedResponse = (): void => {
  mockIPC(() => ({ status: "unexpected" }), { shouldMockEvents: true });
};

export const rejectOpenExternalWithErrorObject = (): void => {
  mockCommands({
    openExternal: () => {
      throw new Error("denied");
    },
  });
};

export const answerChatTurnStreaming = (
  context: "with-context" | "without-context",
): void => {
  mockCommands({
    sendChatMessage: ({ request, onChunk }) => {
      const sent = request.context;
      const wanted =
        context === "with-context"
          ? sent?.symbol === "005930" &&
            sent.market === "Kr" &&
            sent.horizon === "WeeksToMonths"
          : sent === null;
      if (!wanted) throw new Error("unexpected chat context");
      onChunk.onmessage({
        sequence: 1,
        turn_id: request.turn_id,
        chunk: { TextDelta: { text: "holdings " } },
      });
      onChunk.onmessage({
        sequence: 2,
        turn_id: request.turn_id,
        chunk: {
          TextDelta: { text: "[article](https://news.example.test/a)" },
        },
      });
      onChunk.onmessage({
        sequence: 3,
        turn_id: request.turn_id,
        chunk: { Card: { kind: "Signal", id: "signal-1" } },
      });
      onChunk.onmessage({
        sequence: 4,
        turn_id: request.turn_id,
        chunk: {
          Sources: {
            items: [
              {
                kind: "News",
                reference: "news-1",
                url: "https://news.example.test/a",
              },
            ],
          },
        },
      });
      return {
        session_id: request.session_id,
        turn_id: request.turn_id,
        run_id: request.run_id,
        status: "Completed",
        mode: "FreshAnalysis",
        actual_model: null,
        first_token_latency_ms: null,
        resets_at_unix_seconds: null,
        failure_reason: null,
      };
    },
  });
};
