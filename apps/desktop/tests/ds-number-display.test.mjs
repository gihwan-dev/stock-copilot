import assert from "node:assert/strict";
import test from "node:test";
import { componentsDom } from "./ds-components-dom.mjs";

test("DS05 document examples and decimal boundaries preserve core digits", async () => {
  const app = await componentsDom(`
    import {formatNumber,parseMoney,parseRatio,parseQuantity,parseProbability} from LIB_ENTRY;
    window.cleanup=()=>{};window.formatNumber=formatNumber;
    window.parsers={money:parseMoney,percent:parseRatio,quantity:parseQuantity,probability:parseProbability};
    window.invalid=(raw)=>{const value=parseMoney("1","KRW").value;const key=Object.getOwnPropertySymbols(value)[0];return formatNumber({kind:"money",value:{currency:"KRW",[key]:raw}});};
  `);
  try {
    const w = app.open();
    const cases = [
      ["money", "0", "KRW", "compact", "0원"],
      ["money", "9999", "KRW", "compact", "9,999원"],
      ["money", "100000001", "KRW", "compact", "1억 1원"],
      ["money", "100010000", "KRW", "compact", "1억 1만원"],
      ["money", "-12345.60", "KRW", "compact", "−1만 2,345.60원"],
      ["money", "16875000", "KRW", "full", "16,875,000원"],
      ["money", "84312500", "KRW", "full", "84,312,500원"],
      ["money", "182.34", "USD", "full", "$182.34"],
      [
        "money",
        "12345678901234567.89",
        "USD",
        "full",
        "$12,345,678,901,234,567.89",
      ],
      ["money", "1234567898", "KRW", "compact", "12억 3,456만 7,898원"],
      ["money", "84312500", "KRW", "compact", "8,431만 2,500원"],
      ["money", "100000000", "KRW", "compact", "1억원"],
      ["money", "10000.00", "KRW", "compact", "1만 0.00원"],
      ["money", "-850", "KRW", "full", "−850원"],
      ["money", "-0.00", "KRW", "full", "0.00원"],
      ["money", "-0.001", "KRW", "full", "−0.001원"],
      ["money", "0.300000000000000000", "USD", "full", "$0.300000000000000000"],
      ["quantity", "300", null, "full", "300주"],
      ["quantity", "0.123456", null, "full", "0.123456주"],
      ["percent", "48.0", null, "full", "48.0%"],
      ["percent", "15.16", null, "full", "15.16%"],
      ["percent", "2.43", null, "full", "2.43%"],
      ["percent", "-1.12", null, "full", "−1.12%"],
      ["percent", "-0.00", null, "full", "0.00%"],
      ["probability", "30.000", null, "full", "30.000%"],
    ];
    for (const [kind, raw, currency, notation, expected] of cases) {
      const parsed = w.parsers[kind](raw, currency);
      assert.equal(parsed.kind, "ok");
      assert.equal(
        w.formatNumber({ kind, value: parsed.value, notation }).text,
        expected,
        `${kind} ${raw}`,
      );
    }
    for (const raw of [
      "",
      " 1",
      "+1",
      "01",
      "1e3",
      "NaN",
      "1.",
      ".5",
      "1,000",
      "1\n",
    ]) {
      const parsed = w.parsers.money(raw, "KRW");
      assert.equal(parsed.kind, "err", JSON.stringify(raw));
      assert.equal(w.invalid(raw).kind, "unavailable");
      assert.equal(w.invalid(raw).text, "—");
      assert.equal(w.formatNumber({ kind: "unavailable" }).kind, "unavailable");
      assert.equal(w.formatNumber({ kind: "unavailable" }).text, "—");
    }
  } finally {
    app.close();
  }
});

test("DS05 direction overrides numeric signs and preserves rounded zero direction", async () => {
  const app = await componentsDom(`
    import {createElement} from "react";
    import {render,cleanup} from "@testing-library/react";
    import {PriceChange,Money,Percent,Quantity,Probability,DirectionBadge} from DS_ENTRY;
    import {parseMoney,parseRatio,parseQuantity,parseProbability} from LIB_ENTRY;
    window.cleanup=cleanup;
    window.show=(amount,rate,direction)=>render(createElement(PriceChange,{amount:parseMoney(amount,"KRW").value,rate:parseRatio(rate).value,direction}));
    window.examples=()=>[Money,Percent,Quantity,Probability,DirectionBadge].map((component,i)=>render(createElement(component,i===4?{direction:"flat"}:{value:[parseMoney("99.10","USD"),parseRatio("48.0"),parseQuantity("0.123456"),parseProbability("73.2")][i].value})).container.textContent);
  `);
  try {
    const w = app.open();
    for (const [amount, rate, direction, expected, klass, label] of [
      ["1700", "2.43", "up", "▲ +1,700원 (+2.43%)", "text-up", "상승"],
      ["-850", "-1.12", "down", "▼ −850원 (−1.12%)", "text-down", "하락"],
      ["-12", "-3.40", "up", "▲ +12원 (+3.40%)", "text-up", "상승"],
      ["27", "4.50", "down", "▼ −27원 (−4.50%)", "text-down", "하락"],
      ["-0.00", "-0.00", "down", "▼ 0.00원 (0.00%)", "text-down", "하락"],
      ["0.00", "0.00", "up", "▲ 0.00원 (0.00%)", "text-up", "상승"],
      ["5", "6.70", "flat", "– 5원 (6.70%)", "text-ink-muted", "보합"],
    ]) {
      const view = w.show(amount, rate, direction);
      assert.equal(view.container.textContent, expected);
      assert.equal(
        view.getByRole("img", { name: label }).textContent,
        expected.split(" ")[0],
      );
      assert.ok(view.container.querySelector(`.${klass}`));
      assert.equal(
        view.container.querySelector("[aria-label]").getAttribute("aria-label"),
        label,
      );
      assert.ok(
        view.container.querySelector(".tabular-nums.whitespace-nowrap"),
      );
      w.cleanup();
    }
    assert.deepEqual(
      [...w.examples()],
      ["$99.10", "48.0%", "0.123456주", "73.2%", "– 보합"],
    );
  } finally {
    app.close();
  }
});
