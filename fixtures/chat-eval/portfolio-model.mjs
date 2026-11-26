export const AS_OF = "2026-11-26T10:30:00+09:00";

const DEFAULT_PROFILE = {
  source: "saved",
  horizon: "weeks_to_months",
  hours_trading: "disabled",
  risk_appetite: "neutral",
  instrument_weight_cap: "0.2",
  instrument_weight_warning: "0.15",
  sector_weight_cap: "0.4",
  excluded_sectors: [],
  excluded_security_types: [],
};

const label = (text) => ({ untrusted_text: text });

const money = (amount, currency) => ({
  amount: currency === "KRW" ? String(Math.round(amount)) : amount.toFixed(2),
  currency,
});

const ratio = (value) => String(Math.round(value * 10000) / 10000);

const seeded = (text) => {
  let state = 0;
  for (const ch of text) state = (state * 31 + ch.charCodeAt(0)) >>> 0;
  return () => {
    state = (state * 1664525 + 1013904223) >>> 0;
    return state / 0x100000000;
  };
};

const tradingDates = (count) => {
  const dates = [];
  const cursor = new Date(Date.UTC(2026, 10, 25));
  while (dates.length < count) {
    const day = cursor.getUTCDay();
    if (day !== 0 && day !== 6) dates.push(cursor.toISOString().slice(0, 10));
    cursor.setUTCDate(cursor.getUTCDate() - 1);
  }
  return dates;
};

const round = (value, currency) =>
  currency === "KRW" ? Math.round(value) : Math.round(value * 100) / 100;

const candleSeries = (symbol, holding, spec) => {
  const days = spec.days ?? 60;
  const random = seeded(symbol);
  const dailyDrift = (spec.change ?? 0) / days;
  const noise = spec.noise ?? 0.015;
  const currency = holding.currency;
  const items = [];
  let close = holding.price;
  for (const date of tradingDates(days)) {
    const swing = (random() - 0.5) * 2 * noise;
    const open = close / (1 + swing / 2);
    const high = Math.max(open, close) * (1 + random() * noise);
    const low = Math.min(open, close) * (1 - random() * noise);
    items.push({
      trading_date: date,
      open: money(round(open, currency), currency),
      high: money(round(high, currency), currency),
      low: money(round(low, currency), currency),
      close: money(round(close, currency), currency),
      volume: String(Math.round(100000 + random() * 900000)),
    });
    close = close / (1 + dailyDrift + swing);
  }
  return {
    symbol,
    market: holding.market,
    collected_at: AS_OF,
    items,
    next_cursor: null,
  };
};

const currencyOf = (holding) =>
  holding.currency ?? (holding.market === "US" ? "USD" : "KRW");

export const normalizeHoldings = (scenario) =>
  (scenario.account?.holdings ?? []).map((holding) => ({
    ...holding,
    currency: currencyOf(holding),
  }));

export const computeAccount = (scenario) => {
  const fx = scenario.account?.fx === undefined ? 1385 : scenario.account.fx;
  const holdings = normalizeHoldings(scenario);
  const toKrw = (amount, currency) =>
    currency === "KRW" ? amount : fx === null ? null : amount * fx;
  const cash = scenario.account?.cash ?? {};
  const lines = holdings.map((holding) => {
    const valuation = holding.qty * holding.price;
    const purchase = holding.qty * holding.avg;
    return {
      holding,
      valuation,
      purchase,
      profit: valuation - purchase,
      krw: toKrw(valuation, holding.currency),
    };
  });
  const cashKrw = (cash.KRW ?? 0) + (toKrw(cash.USD ?? 0, "USD") ?? 0);
  const total = lines.every((line) => line.krw !== null)
    ? lines.reduce((sum, line) => sum + line.krw, 0) + cashKrw
    : null;
  return { fx, holdings, lines, cash, cashKrw, total };
};

const portfolioResponse = (account) => ({
  account_suffix: "0042",
  recorded_at: AS_OF,
  completeness: account.total === null ? "incomplete" : "complete",
  holdings: [...account.lines]
    .sort((left, right) =>
      left.holding.symbol.localeCompare(right.holding.symbol),
    )
    .map((line) => ({
      symbol: line.holding.symbol,
      name: label(line.holding.name),
      quantity: String(line.holding.qty),
      valuation: money(line.valuation, line.holding.currency),
      purchase: money(line.purchase, line.holding.currency),
      profit: money(line.profit, line.holding.currency),
      weight:
        account.total === null || line.krw === null
          ? null
          : ratio(line.krw / account.total),
    })),
  cash_basis: "buying_power",
  buying_power: [
    { currency: "KRW", amount: money(account.cash.KRW ?? 0, "KRW") },
    { currency: "USD", amount: money(account.cash.USD ?? 0, "USD") },
  ],
  total_assets: {
    value: account.total === null ? null : money(account.total, "KRW"),
    accuracy: "approximate",
  },
  fx_used:
    account.fx === null
      ? null
      : {
          pair: "USD/KRW",
          kind: "mid",
          rate: money(account.fx, "KRW"),
          as_of: AS_OF,
        },
});

const allocationResponse = (account) => {
  if (account.total === null || account.total === 0) {
    return {
      account_suffix: "0042",
      recorded_at: AS_OF,
      completeness: "incomplete",
      lines: [],
      concentration: {
        largest_holding_weight: null,
        top_five_weight: null,
        herfindahl_index: null,
      },
    };
  }
  const buckets = { sector: {}, country: {}, currency: {} };
  for (const line of account.lines) {
    const weight = line.krw / account.total;
    const sector = line.holding.sector ?? "unclassified";
    const country =
      line.holding.country ?? (line.holding.market === "US" ? "US" : "KR");
    buckets.sector[sector] = (buckets.sector[sector] ?? 0) + weight;
    buckets.country[country] = (buckets.country[country] ?? 0) + weight;
    buckets.currency[line.holding.currency] =
      (buckets.currency[line.holding.currency] ?? 0) + weight;
  }
  const lines = [];
  for (const axis of ["sector", "country", "currency"]) {
    for (const [name, weight] of Object.entries(buckets[axis]).sort(
      (left, right) => right[1] - left[1],
    )) {
      lines.push({ axis, label: label(name), weight: ratio(weight) });
    }
  }
  const weights = account.lines
    .map((line) => line.krw / account.total)
    .sort((left, right) => right - left);
  return {
    account_suffix: "0042",
    recorded_at: AS_OF,
    completeness: "complete",
    lines,
    concentration: {
      largest_holding_weight: weights.length ? ratio(weights[0]) : null,
      top_five_weight: weights.length
        ? ratio(weights.slice(0, 5).reduce((sum, value) => sum + value, 0))
        : null,
      herfindahl_index: weights.length
        ? ratio(weights.reduce((sum, value) => sum + value * value, 0))
        : null,
    },
  };
};

const performanceResponse = (spec) => {
  if (spec === undefined || spec === "insufficient") {
    return {
      account_suffix: "0042",
      period: "1y",
      computed_at: AS_OF,
      sample: "insufficient",
      time_weighted_return: null,
      money_weighted_return: null,
      benchmarks: [],
      risk: { annualized_volatility: null, max_drawdown: null },
    };
  }
  return {
    account_suffix: "0042",
    period: spec.period ?? "1y",
    computed_at: AS_OF,
    sample: "sufficient",
    time_weighted_return: ratio(spec.twr),
    money_weighted_return: ratio(spec.mwr ?? spec.twr),
    benchmarks: Object.entries(spec.benchmarks ?? {}).map(
      ([benchmark, value]) => ({ benchmark, period_return: ratio(value) }),
    ),
    risk: {
      annualized_volatility: ratio(spec.vol ?? 0.18),
      max_drawdown: ratio(spec.mdd ?? -0.12),
    },
  };
};

const quoteFor = (symbol, holding, override) => ({
  symbol,
  market: holding.market,
  price: money(override?.price ?? holding.price, holding.currency),
  change_rate: ratio(override?.change_rate ?? 0),
  session: holding.market === "US" ? "closed" : "open",
  collected_at: override?.collected_at ?? AS_OF,
});

const newsResponse = (news) => ({
  collected_at: AS_OF,
  items: (news ?? []).map((item, index) => ({
    handle: `n${String(index + 1).padStart(31, "0")}`,
    symbol: item.symbol ?? null,
    title: label(item.title),
    summary: label(item.summary ?? ""),
    source: item.source ?? "연합뉴스",
    published_at: item.published_at ?? "2026-11-25T18:00:00+09:00",
  })),
  next_cursor: null,
});

const thesisFor = (symbol, spec) => ({
  symbol,
  version: 3,
  status: spec.status ?? "intact",
  narrative: label(spec.narrative),
  claims: (spec.claims ?? []).map((claim, index) => ({
    claim_id: `c${index + 1}`,
    role: index === 0 ? "core" : "supporting",
    statement: label(claim.text),
    verdict: claim.verdict ?? "intact",
  })),
  kill_criteria: (spec.kill ?? []).map((text, index) => ({
    criterion_id: `k${index + 1}`,
    statement: label(text),
    status: "not_met",
  })),
  pending_review_sequences: [],
});

const marketResponse = (spec) => ({
  collected_at: AS_OF,
  indicators: Object.entries(
    spec ?? {
      kospi: { value: 2650.3, change: 0.002 },
      kosdaq: { value: 812.4, change: -0.004 },
      usd_krw: { value: 1385.2, change: 0.001 },
    },
  ).map(([name, value]) => ({
    name,
    value: String(value.value),
    change_rate: ratio(value.change),
  })),
});

export const buildResponses = (scenario) => {
  const account = computeAccount(scenario);
  const responses = {
    get_portfolio: portfolioResponse(account),
    get_allocation: allocationResponse(account),
    get_profile:
      scenario.profile === "default"
        ? { ...DEFAULT_PROFILE, source: "default" }
        : { ...DEFAULT_PROFILE, ...(scenario.profile ?? {}) },
    get_performance: performanceResponse(scenario.performance),
    get_market_overview: marketResponse(scenario.market),
    search_news: newsResponse(scenario.news),
    get_disclosures: { collected_at: AS_OF, items: [], next_cursor: null },
    get_transactions: {
      account_suffix: "0042",
      recorded_at: AS_OF,
      kind: "all",
      items: [],
      next_cursor: null,
    },
  };
  const instruments = {};
  for (const holding of account.holdings) instruments[holding.symbol] = holding;
  for (const [symbol, extra] of Object.entries(scenario.instruments ?? {})) {
    instruments[symbol] = { ...extra, symbol, currency: currencyOf(extra) };
  }
  const quotes = {};
  const candles = {};
  for (const [symbol, holding] of Object.entries(instruments)) {
    quotes[symbol] = quoteFor(symbol, holding, scenario.quotes?.[symbol]);
    candles[symbol] = candleSeries(
      symbol,
      holding,
      scenario.candles?.[symbol] ?? {},
    );
  }
  responses.get_quote = { __bySymbol: quotes };
  responses.get_candles = { __bySymbol: candles };
  const theses = {};
  for (const [symbol, spec] of Object.entries(scenario.thesis ?? {})) {
    theses[symbol] = thesisFor(symbol, spec);
  }
  responses.get_thesis = { __bySymbol: theses };
  for (const [name, payload] of Object.entries(scenario.extra ?? {})) {
    responses[name] = payload;
  }
  return responses;
};
