export const fixtureSource = `
  import {
    parseEntityId,
    parseFreeText,
    parseMoney,
    parsePercentDisplay,
    parseQuantity,
    parseTicker,
    parseTimestamp,
    parseChartDecimal,
  } from LIB_ENTRY;
  import { pendingScreenSources } from IPC_ENTRY;

  const must = (result) => {
    if (result.kind !== "ok") throw new Error("fixture value rejected");
    return result.value;
  };
  window.fx = {
    id: (value) => must(parseEntityId(value)),
    free: (value) => must(parseFreeText(value)),
    krw: (value) => must(parseMoney(value, "KRW")),
    usd: (value) => must(parseMoney(value, "USD")),
    pct: (value) => must(parsePercentDisplay(value)),
    qty: (value) => must(parseQuantity(value)),
    ticker: (value) => must(parseTicker(value)),
    at: (value) => must(parseTimestamp(value)),
    dec: (value) => must(parseChartDecimal(value)),
    ok: (value) => Promise.resolve({ kind: "ok", value }),
    fail: (kind) =>
      Promise.resolve({ kind: "err", error: { kind: "command", error: { kind } } }),
    never: () => new Promise(() => {}),
    sources: (overrides) => ({ ...pendingScreenSources, ...overrides }),
  };
`;

export const waitForText = `
  window.waitText = async (view, needle) => {
    await waitFor(() => {
      if (!(view.container.textContent ?? "").includes(needle)) {
        throw new Error("missing text: " + needle);
      }
    });
    return view.container.textContent ?? "";
  };
`;
