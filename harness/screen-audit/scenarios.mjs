import { getBriefings } from "../../apps/desktop/src/shared/ipc/index.ts";
import { fx, makeSettingsClient, recorded } from "./fixtures.mjs";
import { chatThreads } from "./chat-threads.mjs";
import * as data from "./scenario-data.mjs";

const base = (overrides = {}) => ({
  sources: fx.sources(overrides.sources ?? {}),
  commands: { ...recorded(), ...(overrides.commands ?? {}) },
  settingsClient: overrides.settingsClient,
  setupClient: overrides.setupClient,
});

const setupWith = (state) =>
  makeSettingsClient({ checkTossConnection: () => fx.connection(state) });

const report = (symbol, body, title, url = "https://example.org/report") => ({
  status: "ready",
  asOf: "2026-11-27T08:30:00+09:00",
  entries: [
    {
      symbol,
      body,
      displayName:
        symbol === "012345"
          ? "예시전력"
          : symbol === "005930"
            ? "예시전자"
            : null,
      detail: symbol === "012345" ? "available" : "unavailable",
    },
  ],
  sources: [{ title, url, observedAt: null }],
});

const research = () => ({
  holdings: report(
    "005930",
    "반도체 매출 비중이 높아 메모리 가격 변동이 보유 자산 전체의 손익에 영향을 줍니다 [1].",
    "예시전자 분기보고서",
    null,
  ),
  candidates: report(
    "012345",
    "수주 잔고가 늘었고 보유 기업과 매출처가 다릅니다 [1].\n\n구리 가격 상승분이 납품가에 반영되기까지 이익률이 낮아질 수 있습니다.",
    "예시전력 실적 발표",
  ),
  economy: report(
    null,
    "금리 동결로 기업의 차입 비용은 지난 분기 수준에 머물렀습니다 [1].\n\n설비 투자가 큰 제조업에서는 이자 비용과 신규 수주를 함께 살펴야 합니다.",
    "산업 동향 조사",
  ),
});

const filled = () => ({
  briefings: getBriefings,
  signals: () => fx.ok(data.signals()),
  scorecard: () => fx.ok(data.scorecard()),
  portfolio: () => fx.ok(data.portfolio()),
  allocation: () => fx.ok(data.allocation()),
  instruments: () => fx.ok(data.instruments()),
  instrumentDetail: (symbol) =>
    fx.ok(
      symbol === "012345"
        ? {
            ...data.instrumentDetail(),
            id: fx.id(symbol),
            name: fx.free("예시전력"),
            ticker: fx.ticker(symbol),
            holding: undefined,
          }
        : data.instrumentDetail(),
    ),
  thesis: () => fx.ok(data.thesis()),
  transactions: () => fx.ok(data.transactions()),
  selfCheck: () => fx.ok({ kind: "clear" }),
  status: () => fx.ok(data.status({ kind: "ready" }, { kind: "available" })),
  investorProfile: () => fx.ok(data.profile()),
  orderSettings: () => fx.ok(data.orderSettings()),
});

const everywhere = (load) =>
  Object.fromEntries(Object.keys(filled()).map((key) => [key, load]));

const failing = () => everywhere(() => fx.fail("retryable"));
const loading = () => everywhere(fx.never);

const noBriefings = {
  getBriefings: () => {
    throw { kind: "no-data" };
  },
};

const openDetail = async (helpers) => {
  await helpers.clickFirst('button[aria-label="상세 보기"]');
  await helpers.pause(200);
};

const briefingSessions = () => {
  const { latest, past } = recorded().getBriefings();
  const morning = {
    ...latest,
    body: [
      "밤사이 지표를 확인했습니다 [1]. 원문이 없는 요약도 있습니다 [2].",
      "이번 주 발표 일정과 지난 거래일 종가를 함께 살펴봅니다. ".repeat(5),
      "오늘 아침 점검을 마칩니다.",
    ].join("\n\n"),
  };
  const preMarket = {
    ...latest,
    id: { date: "2026-11-24", kind: "PreUsMarket" },
    finished_at: "2026-11-24T22:30:00+09:00",
    body: [
      "개장 전 지표를 확인했습니다 [1].",
      "지표 발표 순서와 개장 전에 확인할 일정을 정리했습니다. ".repeat(6),
      "미국장 전 점검을 마칩니다.",
    ].join("\n\n"),
    sources: latest.sources.map((source) => ({
      ...source,
      observed_at: "2026-11-24T22:15:00+09:00",
    })),
  };
  return base({
    sources: filled(),
    commands: {
      getBriefings: () => ({ latest: morning, past: [preMarket, ...past] }),
    },
  });
};

export const scenarios = {
  "onboarding-loading": {
    path: "/",
    setup: () =>
      base({
        setupClient: makeSettingsClient({ checkTossConnection: fx.never }),
      }),
  },
  "onboarding-keys": {
    path: "/",
    setup: () => base({ setupClient: setupWith("needs-keys") }),
  },
  "onboarding-invalid-key": {
    path: "/",
    setup: () => base({ setupClient: setupWith("invalid-key") }),
  },
  "onboarding-ip": {
    path: "/",
    setup: () => base({ setupClient: setupWith("ip-not-allowed") }),
  },
  "home-ready": {
    path: "/",
    setup: () =>
      base({
        sources: filled(),
        commands: {
          getDashboardResearch: research,
          getPerformance: () => ({
            lines: ["1m", "3m", "6m", "1y"].map((span, i) =>
              i === 3
                ? {
                    span,
                    status: "insufficient-data",
                    period_return: null,
                    max_drawdown: null,
                    from: null,
                    to: null,
                  }
                : {
                    span,
                    status: "ready",
                    period_return: {
                      rate: ["2.4", "3.6", "5.8"][i],
                      direction: "up",
                    },
                    max_drawdown: { rate: "-1.2", direction: "down" },
                    from: "2026-05-27T00:00:00Z",
                    to: "2026-11-27T00:00:00Z",
                  },
            ),
          }),
        },
      }),
    act: (helpers) => helpers.waitText("예시바이오"),
  },
  "home-research-refresh": {
    path: "/",
    setup: () => {
      let reads = 0;
      return base({
        sources: filled(),
        commands: {
          getDashboardResearch: () => {
            reads += 1;
            const data = research();
            if (reads >= 3)
              return {
                ...data,
                candidates: {
                  ...data.candidates,
                  entries: data.candidates.entries.map((entry) => ({
                    ...entry,
                    detail: "unavailable",
                  })),
                },
              };
            return {
              holdings: { ...data.holdings, status: "failed" },
              candidates: {
                status: "empty",
                entries: [],
                sources: [],
                asOf: null,
              },
              economy: {
                ...data.economy,
                status: reads === 1 ? "refreshing" : "ready",
              },
            };
          },
        },
      });
    },
  },
  "home-empty": { path: "/", setup: () => base({ commands: noBriefings }) },
  "home-loading": { path: "/", setup: () => base({ sources: loading() }) },
  "home-error": { path: "/", setup: () => base({ sources: failing() }) },
  "home-self-check": {
    path: "/",
    setup: () =>
      base({
        sources: {
          ...filled(),
          selfCheck: () =>
            fx.ok({ kind: "problem", problem: "ip-not-allowed" }),
        },
      }),
  },
  "home-update-ready": {
    path: "/",
    setup: () =>
      base({
        sources: filled(),
        commands: {
          getUpdateStatus: () => data.updateReady,
          restartToUpdate: () => data.updateReady,
        },
      }),
  },
  "update-sheet": {
    path: "/",
    setup: () =>
      base({
        sources: filled(),
        commands: {
          getUpdateStatus: () => data.updateReady,
          restartToUpdate: () => data.updateReady,
        },
      }),
    act: async (helpers) => {
      await helpers.click("재시작");
      await helpers.waitText("새 버전으로 다시 시작할까요?");
      await helpers.pause(200);
    },
  },
  "instrument-detail": {
    path: "/",
    setup: () => base({ sources: filled() }),
    act: openDetail,
  },
  "instrument-detail-loading": {
    path: "/",
    setup: () =>
      base({
        sources: { ...filled(), instrumentDetail: fx.never, thesis: fx.never },
      }),
    act: openDetail,
  },
  "instrument-detail-error": {
    path: "/",
    setup: () =>
      base({
        sources: {
          ...filled(),
          instrumentDetail: () => fx.fail("retryable"),
          thesis: () => fx.fail("retryable"),
        },
      }),
    act: openDetail,
  },
  "briefing-latest": {
    path: "/briefing",
    setup: briefingSessions,
    act: (helpers) => helpers.waitText("미국장 전 점검을 마칩니다."),
  },
  "chat-threads": {
    path: "/chat",
    setup: () =>
      base({
        sources: filled(),
        commands: chatThreads(
          () => new Promise((resolve) => setTimeout(resolve, 300)),
        ),
      }),
    act: (helpers) => helpers.waitText("저장된 예시 답변입니다."),
  },
  "chat-long": {
    path: "/chat",
    setup: () => {
      const history = recorded().getChatHistory();
      const entries = Array.from({ length: 36 }, (_, index) =>
        history.entries.map((entry) => ({
          ...entry,
          turn_id: `turn-${index + 1}`,
          text: `대화 ${index + 1} — ${entry.text}`,
        })),
      ).flat();
      return base({
        sources: filled(),
        commands: {
          getChatHistory: () => ({ ...history, entries, active_turn: null }),
          sendChatMessage: ({ request, onChunk }) => {
            onChunk.onmessage({
              turn_id: request.turn_id,
              sequence: 1,
              chunk: { TextDelta: { text: "스크롤 확인용 새 응답입니다." } },
            });
            return {
              ...recorded().sendChatMessage(),
              turn_id: request.turn_id,
              session_id: request.session_id,
              run_id: request.run_id,
            };
          },
        },
      });
    },
    act: (helpers) => helpers.waitText("대화 36 — 보유 종목을 확인했습니다."),
  },
  "chat-ready": {
    path: "/chat",
    setup: () =>
      base({
        sources: filled(),
        commands: {
          getChatHistory: () => ({
            session_id: "main-chat",
            entries: [],
            active_turn: null,
          }),
        },
      }),
    act: (helpers) => helpers.waitText("무엇이든 물어보세요"),
  },
  "briefings-ready": {
    path: "/briefings",
    setup: briefingSessions,
    act: (helpers) => helpers.waitText("미국장 전 점검을 마칩니다."),
  },
  "briefings-empty": {
    path: "/briefings",
    setup: () =>
      base({
        sources: { briefings: () => fx.ok({ latest: undefined, past: [] }) },
      }),
  },
  "briefings-error": {
    path: "/briefings",
    setup: () => base({ sources: { briefings: () => fx.fail("retryable") } }),
  },
  "transactions-ready": {
    path: "/transactions",
    setup: () => base({ sources: filled() }),
  },
  "transactions-empty": {
    path: "/transactions",
    setup: () =>
      base({
        sources: {
          transactions: () =>
            fx.ok({ recordedAt: fx.at("2026-11-25T09:00:00+09:00"), rows: [] }),
        },
      }),
  },
  "transactions-error": {
    path: "/transactions",
    setup: () =>
      base({ sources: { transactions: () => fx.fail("retryable") } }),
  },
  "settings-basic": {
    path: "/settings",
    setup: () =>
      base({ sources: filled(), settingsClient: makeSettingsClient() }),
  },
  "settings-advanced": {
    path: "/settings",
    setup: () =>
      base({ sources: filled(), settingsClient: makeSettingsClient() }),
    act: async (helpers) => {
      await helpers.click("고급");
      await helpers.pause(300);
    },
  },
  "settings-update-ready": {
    path: "/settings",
    setup: () =>
      base({
        sources: filled(),
        commands: { getUpdateStatus: () => data.updateReady },
      }),
  },
  "status-ready": {
    path: "/settings/status",
    setup: () => base({ sources: filled() }),
  },
  "status-waiting": {
    path: "/settings/status",
    setup: () =>
      base({
        sources: {
          status: () =>
            fx.ok(
              data.status(
                {
                  kind: "unavailable",
                  reason: fx.free("Claude Code 버전이 바뀌어 다시 확인 중"),
                },
                {
                  kind: "waiting",
                  resumesAt: fx.at("2026-11-25T13:00:00+09:00"),
                },
              ),
            ),
        },
      }),
  },
  "status-error": {
    path: "/settings/status",
    setup: () => base({ sources: { status: () => fx.fail("retryable") } }),
  },
};
