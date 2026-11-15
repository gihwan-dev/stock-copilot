import assert from "node:assert/strict";
import test from "node:test";
import { componentsDom } from "./ds-components-dom.mjs";
const setup = () =>
  componentsDom(`
  import {createElement} from "react";
  import {render,cleanup} from "@testing-library/react";
  import {Money,Percent,Probability,PriceChange,DirectionBadge,Quantity} from DS_ENTRY;
  import {parseMoney,parsePrice,parsePercentDisplay,parseQuantity,parseRatio,parseProbability} from LIB_ENTRY;
  window.cleanup=cleanup;
  const value=(parse,raw,currency)=>{const result=parse(raw,currency);if(result.kind==="ok")return result.value;const safe=parse("1",currency==="USD"?"USD":"KRW").value;return {...safe,currency,[Object.getOwnPropertySymbols(safe)[0]]:raw};};
  window.number=(kind,raw,currency="KRW",direction,notation)=>{
    const types={Money,Percent,Probability,Quantity,Price:Money};
    const parsers={Money:parseMoney,Price:parsePrice,Percent:parsePercentDisplay,Probability:parsePercentDisplay,Quantity:parseQuantity};
    return render(createElement(types[kind],{value:value(parsers[kind],raw,currency),direction,notation}));
  };
  window.change=(amount,rate,direction)=>render(createElement(PriceChange,{amount:value(parseMoney,amount,"KRW"),rate:value(parsePercentDisplay,rate),direction}));
  window.raw=(value)=>render(createElement(Money,{value}));
  window.domain=(kind)=>render(createElement(kind==="ratio"?Percent:Probability,{value:(kind==="ratio"?parseRatio:parseProbability)("0.1516").value}));
  window.badge=(direction)=>render(createElement(DirectionBadge,{direction}));
`);
const invalid = (view) => {
  assert.equal(view.container.textContent, "—");
  const node = view.getByRole("img", { name: "값을 표시할 수 없음" });
  assert.ok(node.classList.contains("text-alert"));
  assert.ok(node.classList.contains("tabular-nums"));
  assert.ok(node.classList.contains("whitespace-nowrap"));
};
test("DS05 review rejects inconsistent directions for the whole change", async () => {
  const app = await setup();
  try {
    const w = app.open();
    for (const [amount, rate, direction] of [
      ["-1700", "-2.43", "up"],
      ["27", "4.50", "down"],
      ["5", "6.70", "flat"],
      ["-5", "0.00", "flat"],
      ["0", "-2", "up"],
      ["-2", "0", "up"],
      ["-2", "-1", "flat"],
      ["1", "2", "other"],
      ["1", "2", undefined],
      ["1", "2", null],
    ]) {
      invalid(w.change(amount, rate, direction));
      w.cleanup();
    }
    for (const direction of ["up", "down", "flat"]) {
      const view = w.change("-0.00", "0.000", direction);
      assert.equal(
        view.container.textContent,
        { up: "▲", down: "▼", flat: "–" }[direction] + " 0.00원 (0.000%)",
      );
      w.cleanup();
    }
  } finally {
    app.close();
  }
});
test("DS05 review standalone directional amounts percentages and prices", async () => {
  const app = await setup();
  try {
    const w = app.open();
    for (const [kind, raw, currency, direction, expected, color] of [
      ["Money", "1234000", "KRW", "up", "▲ +1,234,000원", "text-up"],
      ["Money", "-182.34", "USD", "down", "▼ −$182.34", "text-down"],
      ["Money", "-182.34", "USD", undefined, "−$182.34", null],
      ["Percent", "5.9", "KRW", "up", "▲ +5.9%", "text-up"],
      ["Percent", "-1.20", "KRW", "down", "▼ −1.20%", "text-down"],
      ["Price", "182.34", "USD", undefined, "$182.34", null],
      ["Price", "70000", "KRW", undefined, "70,000원", null],
      ["Money", "12345", "KRW", undefined, "12,345원", null],
      ["Quantity", "0.123456", "KRW", undefined, "0.123456주", null],
      ["Probability", "64", "KRW", undefined, "64%", null],
    ]) {
      const view = w.number(kind, raw, currency, direction);
      assert.equal(view.container.textContent, expected);
      assert.ok(
        view.container.querySelector(".tabular-nums.whitespace-nowrap"),
      );
      if (color) assert.ok(view.container.querySelector("." + color));
      w.cleanup();
    }
    for (const [kind, raw, direction] of [
      ["Money", "-5", "up"],
      ["Money", "5", "down"],
      ["Money", "-5", "flat"],
      ["Percent", "5", "flat"],
      ["Money", "5", "other"],
      ["Percent", "5", "other"],
    ]) {
      invalid(w.number(kind, raw, "KRW", direction));
      w.cleanup();
    }
  } finally {
    app.close();
  }
});
test("DS05 review probability integer boundaries and malformed presentation", async () => {
  const app = await setup();
  try {
    const w = app.open();
    for (const raw of ["1", "64", "99"]) {
      assert.equal(
        w.number("Probability", raw).container.textContent,
        raw + "%",
      );
      w.cleanup();
    }
    for (const raw of ["0", "100", "64.5", "1.0", "-1", "NaN", "1\n"]) {
      invalid(w.number("Probability", raw));
      w.cleanup();
    }
    for (const kind of ["Money", "Price", "Percent", "Quantity"]) {
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
        invalid(w.number(kind, raw));
        w.cleanup();
      }
    }
    invalid(w.change("1700", "NaN", "up"));
    w.cleanup();
    for (const direction of ["other", undefined, null]) {
      invalid(w.badge(direction));
      w.cleanup();
    }
    for (const value of [null, undefined, {}, "5", 5]) {
      invalid(w.raw(value));
      w.cleanup();
    }
    invalid(w.number("Money", "12", "EUR"));
    w.cleanup();
    for (const kind of ["ratio", "probability"]) {
      invalid(w.domain(kind));
      w.cleanup();
    }
    for (const direction of ["up", "down", "flat"]) {
      const view = w.badge(direction);
      assert.equal(
        view.container
          .querySelector("[aria-hidden]")
          .getAttribute("aria-hidden"),
        "true",
      );
      assert.equal(
        view.container.textContent,
        { up: "▲ 상승", down: "▼ 하락", flat: "– 보합" }[direction],
      );
      assert.ok(
        view.container.querySelector(
          "." +
            { up: "text-up", down: "text-down", flat: "text-ink-muted" }[
              direction
            ],
        ),
      );
      w.cleanup();
    }
  } finally {
    app.close();
  }
});
test("DS05 review compact is exact KRW integers with trillion grouping", async () => {
  const app = await setup();
  try {
    const w = app.open();
    for (const [raw, currency, expected] of [
      ["10000", "KRW", "1만원"],
      ["99999", "KRW", "9만 9,999원"],
      ["100000", "KRW", "10만원"],
      ["1000000000000", "KRW", "1조원"],
      ["1234567890123", "KRW", "1조 2,345억 6,789만 123원"],
      ["1234567890123456", "KRW", "1,234조 5,678억 9,012만 3,456원"],
      ["84312500", "USD", "$84,312,500"],
      ["10000.00", "KRW", "10,000.00원"],
      ["12345.60", "KRW", "12,345.60원"],
      ["-1000000000000", "KRW", "−1조원"],
    ]) {
      assert.equal(
        w.number("Money", raw, currency, undefined, "compact").container
          .textContent,
        expected,
      );
      w.cleanup();
    }
  } finally {
    app.close();
  }
});
