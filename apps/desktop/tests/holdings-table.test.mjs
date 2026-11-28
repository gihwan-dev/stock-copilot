import assert from "node:assert/strict";
import { join } from "node:path";
import test from "node:test";
import { desktopRoot } from "./paths.mjs";
import { screenDom } from "./screen-dom.mjs";
import { fixtureSource } from "./screen-fixtures.mjs";

const card = join(desktopRoot, "src/features/portfolio/ui/HoldingsCard.tsx");
const source = `
  import { createElement } from "react";
  import { render, cleanup, fireEvent } from "@testing-library/react";
  import { HoldingsCard } from ${JSON.stringify(card)};
  ${fixtureSource}
  window.cleanup = cleanup;
  window.select = (instrument) => fireEvent.click(instrument);
  window.show = (direction) => render(createElement(HoldingsCard, {
    holdings: [{
      id: fx.id("security-15"), name: fx.free("예시 바이오"),
      ticker: fx.free("EXBI"), quantity: fx.qty("7"), value: fx.usd("795.54"),
      profit: { amount: fx.usd("-27.02"), rate: fx.pct("-3.28"), direction },
      trading: "suspended",
    }],
    onOpenInstrument: (id) => { window.openedInstrument = id; },
    onOpenTransactions: () => {},
  }));
`;
test("보유 표에서 종목명과 코드를 함께 읽고 평가손익과 수익률을 비교한다", async () => {
  const bundle = await screenDom(source);
  try {
    const w = bundle.open();
    const view = w.show("down");
    assert.deepEqual(
      Array.from(view.getAllByRole("columnheader"), (node) => node.textContent),
      ["종목", "수량", "평가금액", "평가손익"],
    );
    const instrument = view.getByRole("button", {
      name: "예시 바이오",
      description: "거래정지",
    });
    const identity = instrument.closest('th[scope="row"]');
    assert.ok(identity);
    assert.ok(identity.contains(instrument));
    assert.ok(identity.contains(view.getByText("EXBI")));
    assert.ok(identity.contains(view.getByText("거래정지")));
    const pnl = view.getByRole("group", { name: "하락" });
    assert.ok(pnl.contains(view.getByText("−$27.02")));
    assert.ok(pnl.contains(view.getByText("−3.28%")));
    assert.equal(view.getByText("$795.54").closest("td").cellIndex, 2);
    w.select(instrument);
    assert.equal(w.openedInstrument, "security-15");
    w.cleanup();
    for (const inconsistent of ["up", undefined]) {
      const missing = w.show(inconsistent);
      assert.equal(
        missing.getByRole("img", { name: "값을 표시할 수 없음" }).closest("td")
          .textContent,
        "—",
      );
      w.cleanup();
    }
  } finally {
    bundle.close();
  }
});
