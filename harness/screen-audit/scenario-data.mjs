import { fx } from "./fixtures.mjs";

const signed = (amount, rate, direction) => ({
  amount: fx.krw(amount),
  rate: fx.pct(rate),
  direction,
});

export const portfolio = () => ({
  recordedAt: fx.at("2026-11-25T09:30:00+09:00"),
  connection: "live",
  totalApproximate: fx.krw("123456789"),
  exchangeRate: { kind: "mid-rate", at: fx.at("2026-11-25T09:00:00+09:00") },
  currencies: [
    {
      currency: "KRW",
      holdings: fx.krw("100000000"),
      buyingPower: fx.krw("3456789"),
    },
    {
      currency: "USD",
      holdings: fx.usd("14000.50"),
      buyingPower: fx.usd("120.25"),
    },
  ],
  holdings: [
    {
      id: fx.id("holding-1"),
      name: fx.free("예시전자"),
      ticker: fx.ticker("005930"),
      quantity: fx.qty("12"),
      value: fx.krw("850000"),
      profit: signed("50000", "6.25", "up"),
    },
    {
      id: fx.id("holding-2"),
      name: fx.free("예시바이오"),
      ticker: fx.ticker("207940"),
      quantity: fx.qty("3"),
      value: fx.krw("2400000"),
      profit: signed("-120000", "-4.76", "down"),
    },
  ],
  today: signed("-15000", "-0.12", "down"),
});

export const allocation = () => ({
  instruments: [
    {
      id: fx.id("a-1"),
      label: fx.free("예시바이오"),
      value: fx.krw("2400000"),
      weight: fx.pct("73.8"),
      level: "over-cap",
    },
    {
      id: fx.id("a-2"),
      label: fx.free("예시전자"),
      value: fx.krw("850000"),
      weight: fx.pct("26.2"),
      level: "normal",
    },
  ],
  sectors: [
    {
      id: fx.id("s-1"),
      label: fx.free("헬스케어"),
      value: fx.krw("2400000"),
      weight: fx.pct("73.8"),
      level: "warning",
    },
  ],
  concentration: "concentrated",
});

export const instruments = () => ({
  rows: [
    {
      id: fx.id("i-1"),
      name: fx.free("예시전자"),
      ticker: fx.ticker("005930"),
      origin: "holding",
      todayRate: fx.pct("1.25"),
      direction: "up",
    },
    {
      id: fx.id("i-2"),
      name: fx.free("예시바이오"),
      ticker: fx.ticker("207940"),
      origin: "watchlist",
      todayRate: fx.pct("-0.50"),
      direction: "down",
    },
  ],
});

export const instrumentDetail = () => ({
  id: fx.id("i-1"),
  name: fx.free("예시전자"),
  ticker: fx.ticker("005930"),
  price: fx.krw("71200"),
  todayRate: fx.pct("1.25"),
  direction: "up",
  quotedAt: fx.at("2026-11-25T10:00:00+09:00"),
  candles: [
    ["c-1", "11-21", "69000", "70100", "68800", "69900"],
    ["c-2", "11-24", "70000", "71500", "69800", "70300"],
    ["c-3", "11-25", "70300", "71400", "70100", "71200"],
  ].map(([id, label, open, high, low, close]) => ({
    id: fx.id(id),
    label: fx.free(label),
    open: fx.dec(open),
    high: fx.dec(high),
    low: fx.dec(low),
    close: fx.dec(close),
  })),
  holding: {
    quantity: fx.qty("12"),
    value: fx.krw("854400"),
    profit: fx.krw("54400"),
    profitRate: fx.pct("6.80"),
  },
});

export const thesis = () => ({
  claims: [
    {
      id: fx.id("cl-1"),
      text: fx.free("메모리 가격 반등"),
      status: "confirmed",
    },
    {
      id: fx.id("cl-2"),
      text: fx.free("웹에서 본 수주 소식"),
      status: "needs-review",
    },
  ],
  history: [
    {
      id: fx.id("h-1"),
      at: fx.at("2026-11-24T20:00:00+09:00"),
      summary: fx.free("수주 주장 추가"),
    },
  ],
});

const card = (overrides) => ({
  id: fx.id("card-1"),
  name: fx.free("예시전자"),
  ticker: fx.ticker("005930"),
  direction: "buy",
  market: "open",
  orderSwitch: "on",
  proposal: fx.id("proposal-1"),
  price: fx.krw("71200"),
  sizing: {
    kind: "sized",
    quantity: fx.qty("3"),
    amount: fx.krw("213600"),
    costs: fx.krw("320"),
    weightAfter: fx.pct("4.2"),
  },
  aiProbability: fx.pct("46"),
  trackedProbability: fx.pct("41"),
  threshold: fx.pct("38"),
  binHitRate: fx.pct("44"),
  binCount: fx.dec("12"),
  rationale: fx.free("3분기 메모리 가격이 올라 영업이익이 늘 가능성이 커요."),
  counterargument: fx.free("원화 강세가 이어지면 수출 이익이 줄 수 있어요."),
  sources: [
    { id: fx.id("s-1"), kind: "tool", label: fx.free("시세 조회") },
    { id: fx.id("s-2"), kind: "news", label: fx.free("예시 뉴스 기사") },
  ],
  ...overrides,
});

export const signals = () => ({
  cards: [
    card({}),
    card({
      id: fx.id("card-2"),
      name: fx.free("예시바이오"),
      ticker: fx.ticker("207940"),
      direction: "sell",
      proposal: fx.id("proposal-2"),
      price: fx.krw("800000"),
      sizing: {
        kind: "sized",
        quantity: fx.qty("1"),
        amount: fx.krw("800000"),
        costs: fx.krw("1200"),
        weightAfter: fx.pct("12.5"),
      },
    }),
  ],
});

const combination = (id, model, verdict, judged) => ({
  id: fx.id(id),
  model: fx.free(model),
  horizon: fx.free("며칠"),
  judged: fx.dec(judged),
  verdict,
  brier: verdict === "ready" ? fx.dec("0.214") : undefined,
  bins: [
    ["b1", "30~40", "35", "33", "12"],
    ["b2", "40~50", "45", "47", "21"],
  ].map(([bin, label, mean, hit, count]) => ({
    id: fx.id(`${id}-${bin}`),
    label: fx.free(label),
    meanPrediction: fx.dec(mean),
    hitRate: fx.dec(hit),
    count: fx.dec(count),
  })),
  signalReturn: verdict === "ready" ? fx.pct("3.4") : undefined,
  benchmarkReturn: verdict === "ready" ? fx.pct("1.2") : undefined,
  intervalLow: verdict === "ready" ? fx.pct("-0.8") : undefined,
  intervalHigh: verdict === "ready" ? fx.pct("7.1") : undefined,
  comparisons: fx.dec("3"),
});

export const scorecard = () => ({
  combinations: [
    combination("c-1", "Opus", "ready", "42"),
    combination("c-2", "Sonnet", "pending", "12"),
  ],
});

export const transactions = () => ({
  recordedAt: fx.at("2026-11-25T09:00:00+09:00"),
  rows: [
    {
      id: fx.id("tx-1"),
      settledAt: fx.at("2026-11-20T00:00:00+09:00"),
      name: fx.free("예시전자"),
      kind: "buy",
      quantity: fx.qty("5"),
      amount: fx.krw("350000"),
      presence: "present",
    },
    {
      id: fx.id("tx-2"),
      settledAt: fx.at("2026-11-21T00:00:00+09:00"),
      name: fx.free("예시배당주"),
      kind: "dividend",
      quantity: undefined,
      amount: fx.krw("12000"),
      presence: "vanished",
    },
  ],
});

export const status = (aiRuntime, aiLimit) => ({
  records: [
    {
      id: fx.id("r-1"),
      at: fx.at("2026-11-25T08:00:00+09:00"),
      check: fx.free("AI 실행 환경"),
      detail: fx.free("격리 설정 확인에 실패했어요"),
    },
  ],
  aiRuntime,
  aiLimit,
  sources: [
    {
      id: fx.id("src-1"),
      name: fx.free("연합뉴스"),
      lastSuccess: fx.at("2026-11-25T09:10:00+09:00"),
    },
    { id: fx.id("src-2"), name: fx.free("SEC EDGAR"), lastSuccess: undefined },
  ],
  aiUsage: [
    {
      id: fx.id("u-1"),
      task: fx.free("확률 추정"),
      tokens: fx.dec("12345"),
      seconds: fx.dec("42"),
    },
  ],
});

export const profile = () => ({
  horizon: "few-days",
  riskAppetite: "conservative",
  instrumentCap: "15",
  sectorCap: "30",
  excludedTypes: ["etf"],
});

export const orderSettings = () => ({
  marketOrders: "on",
  hoursHorizon: "off",
  cardExpiry: "15",
  orderCountLimit: "5",
});

export const updateReady = {
  current_version: "0.1.0",
  state: {
    ready: { version: "0.2.0", notes: "- 자동 업데이트를 넣었어요" },
  },
};
