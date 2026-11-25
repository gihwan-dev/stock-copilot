import { getBriefings } from "../../apps/desktop/src/shared/ipc/index.ts";
import { fx, makeSettingsClient, recorded } from "./fixtures.mjs";
import * as data from "./scenario-data.mjs";

const base = (overrides = {}) => ({
  sources: fx.sources(overrides.sources ?? {}),
  commands: { ...recorded(), ...(overrides.commands ?? {}) },
  settingsClient: overrides.settingsClient,
  setupClient: overrides.setupClient,
});

const setupWith = (state) =>
  makeSettingsClient({ checkTossConnection: () => fx.connection(state) });

const filled = () => ({
  briefings: getBriefings,
  signals: () => fx.ok(data.signals()),
  scorecard: () => fx.ok(data.scorecard()),
  portfolio: () => fx.ok(data.portfolio()),
  allocation: () => fx.ok(data.allocation()),
  instruments: () => fx.ok(data.instruments()),
  instrumentDetail: () => fx.ok(data.instrumentDetail()),
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
    setup: () => base({ sources: filled() }),
    act: (helpers) => helpers.waitText("예시바이오"),
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
  "briefings-ready": {
    path: "/briefings",
    setup: () => base({ sources: filled() }),
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
