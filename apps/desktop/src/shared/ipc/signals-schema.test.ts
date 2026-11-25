import { expect, test } from "vitest";
import { wire } from "./schema";
import { settleApproval, settleSignals, settleSkip } from "./signals-schema";
import { responses } from "./testing/responses";

const card = {
  symbol: "005930",
  name: "Samsung",
  direction: "buy",
  market: "closed",
  order_switch: "off",
  price: { amount: "71400", currency: "KRW" },
  ai_probability: "62.0",
  tracked_probability: "58.5",
  threshold: "55.0",
  bin_hit_rate: null,
  bin_count: "0",
  rationale: "memory",
  counterargument: "foundry",
  sources: [
    { kind: "news", label: "headline" },
    { kind: "tool", label: "tool:get_quote" },
  ],
  proposal: null,
};

test("settleSignals keeps the recorded empty list", () => {
  expect(
    settleSignals({ status: "ok", data: wire(responses.getSignals) }),
  ).toStrictEqual({ kind: "ok", value: { cards: [] } });
});

test("settleSignals turns a card into a pending-size view without an order proposal", () => {
  const result = settleSignals({ status: "ok", data: wire({ cards: [card] }) });
  expect(result.kind).toBe("ok");
  if (result.kind !== "ok") return;
  const [view] = result.value.cards;
  expect(view?.sizing).toStrictEqual({ kind: "pending" });
  expect(view?.proposal === undefined).toBe(true);
  expect(view?.binHitRate === undefined).toBe(true);
  expect(view?.sources.map((source) => source.kind)).toStrictEqual([
    "news",
    "tool",
  ]);
});

test("settleSignals refuses an unknown direction and settleSkip keeps the recorded reply", () => {
  expect(
    settleSignals({
      status: "ok",
      data: wire({ cards: [{ ...card, direction: "hold" }] }),
    }).kind,
  ).toBe("err");
  expect(
    settleSkip({ status: "ok", data: wire(responses.skipSignal) }),
  ).toStrictEqual({ kind: "ok", value: "recorded" });
});

test("settleSignals carries the stored proposal so the card can offer the order button", () => {
  const result = settleSignals({
    status: "ok",
    data: wire({
      cards: [{ ...card, proposal: "p-0123abcd-1795566600000000000" }],
    }),
  });
  expect(result.kind).toBe("ok");
  if (result.kind !== "ok") return;
  expect(result.value.cards[0]?.proposal).toBe(
    "p-0123abcd-1795566600000000000",
  );
});

test("settleApproval treats only a sent order as requested", () => {
  expect(
    settleApproval({
      status: "ok",
      data: wire({ status: "submitted", reason: "none" }),
    }),
  ).toStrictEqual({ kind: "ok", value: "requested" });
  expect(
    settleApproval({
      status: "ok",
      data: wire({ status: "rejected", reason: "switch-off" }),
    }).kind,
  ).toBe("err");
});
