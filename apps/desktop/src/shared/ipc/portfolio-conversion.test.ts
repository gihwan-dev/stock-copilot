import { expect, test } from "vitest";
import { parseMoney } from "../lib";
import { settlePortfolio } from "./portfolio-schema";
import { wire } from "./schema";
import { responses } from "./testing/responses";

const recorded = responses.getPortfolio;
const rateAt = "2026-11-25T08:52:00+09:00";
const conversion = {
  krw_per_usd: { amount: "1345.67", currency: "KRW" },
  holdings_krw: { amount: "3370", currency: "KRW" },
  buying_power_krw: { amount: "4037", currency: "KRW" },
};
const converted = {
  ...recorded,
  exchange_rate_at: rateAt,
  usd_conversion: conversion,
  currencies: [
    {
      currency: "KRW",
      holdings: { amount: "1230", currency: "KRW" },
      buying_power: { amount: "770", currency: "KRW" },
    },
    {
      currency: "USD",
      holdings: { amount: "2.50", currency: "USD" },
      buying_power: { amount: "3.00", currency: "USD" },
    },
  ],
  total: { amount: "9407", currency: "KRW" },
};

test("carries the applied fractional rate and converted KRW amounts", () => {
  const result = settlePortfolio({ status: "ok", data: wire(converted) });
  expect(result.kind).toBe("ok");
  if (result.kind !== "ok") return;
  expect(result.value.exchangeRate?.at).toBe(rateAt);
  expect({
    kind: "ok",
    value: result.value.usdConversion?.krwPerUsd,
  }).toStrictEqual(parseMoney("1345.67", "KRW"));
  expect({
    kind: "ok",
    value: result.value.usdConversion?.holdingsKrw,
  }).toStrictEqual(parseMoney("3370", "KRW"));
  expect({
    kind: "ok",
    value: result.value.usdConversion?.buyingPowerKrw,
  }).toStrictEqual(parseMoney("4037", "KRW"));
});

test.each([
  { ...converted, total_coverage: "partial" },
  { ...converted, exchange_rate_at: null },
  {
    ...converted,
    usd_conversion: {
      ...conversion,
      krw_per_usd: { amount: "0.00", currency: "KRW" },
    },
  },
  {
    ...converted,
    usd_conversion: {
      ...conversion,
      holdings_krw: { amount: "3370", currency: "USD" },
    },
  },
  { ...converted, usd_conversion: { ...conversion, buying_power_krw: null } },
])(
  "rejects inconsistent or malformed USD conversion evidence %#",
  (payload) => {
    expect(
      settlePortfolio({ status: "ok", data: wire(payload) }),
    ).toStrictEqual({
      kind: "err",
      error: { kind: "invalid-response" },
    });
  },
);

test("leaves a legacy time without an applied rate undisplayed", () => {
  const result = settlePortfolio({
    status: "ok",
    data: wire({
      ...recorded,
      usd_conversion: undefined,
      exchange_rate_at: rateAt,
    }),
  });
  expect(result.kind).toBe("ok");
  if (result.kind !== "ok") return;
  expect(result.value.usdConversion).toBeUndefined();
  expect(result.value.exchangeRate).toBeUndefined();
});
