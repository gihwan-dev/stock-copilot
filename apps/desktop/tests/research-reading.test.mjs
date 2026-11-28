import assert from "node:assert/strict";
import { test } from "node:test";
import { join } from "node:path";
import { desktopRoot } from "./paths.mjs";
import { screenDom } from "./screen-dom.mjs";

test("the dashboard preserves an expanded report as one list and opens original numbered sources", async () => {
  const research = join(
    desktopRoot,
    "src/features/research/ui/DashboardResearch.tsx",
  );
  const bundle = await screenDom(`
    import {createElement as h} from "react";
    import {act,render,fireEvent,cleanup,waitFor} from "@testing-library/react";
    import {QueryClientProvider} from "@tanstack/react-query";
    import {createAppQueryClient} from QUERY_ENTRY;
    import {mockCommands} from MOCK_IPC_ENTRY;
    import {useDashboardResearch} from ${JSON.stringify(research)};
    window.scrolled = [];
    HTMLElement.prototype.scrollIntoView = function(options){window.scrolled.push([this.id,options.block]);};
    window.opened = [];
    window.focusCitation = (node) => act(() => node.focus());
    window.click = fireEvent.click;
    window.cleanup = cleanup;
    window.wait = waitFor;
    window.show = (body, sources) => {
      const report = {status:"ready",asOf:null,entries:[{symbol:null,body}],sources};
      mockCommands({getDashboardResearch:()=>({holdings:report,candidates:report,economy:report}),openExternal:({request})=>{window.opened.push(request.url);return {status:"opened"};}});
      const Report = () => useDashboardResearch(()=>{})("economy");
      return render(h(QueryClientProvider,{client:createAppQueryClient()},h(Report)));
    };
  `);
  try {
    const w = bundle.open();
    const sources = Array.from({ length: 26 }, (_, i) => ({
      title: `공개 자료 ${i + 1}`,
      url: i === 1 ? "https://example.org/observation" : null,
      observedAt: null,
    }));
    const lines = sources.map((_, i) => `관측한 내용입니다 [${i + 1}].`);
    const old = w.show(lines.join("\n"), sources);
    const more = (await old.findByText("20개")).closest("button");
    assert.equal(more.getAttribute("aria-expanded"), "false");
    assert.equal(old.getAllByRole("listitem").length, 6);
    w.click(more, { detail: 0 });
    await w.wait(() =>
      assert.equal(w.document.activeElement.textContent, "[7]"),
    );
    assert.equal(old.getAllByRole("list").length, 1);
    assert.equal(old.getAllByRole("listitem").length, 26);
    const less = old.getByText("접기", { exact: true }).closest("button");
    assert.ok(
      old.getByRole("list").compareDocumentPosition(less) &
        w.Node.DOCUMENT_POSITION_FOLLOWING,
    );
    w.click(less);
    assert.equal(old.getAllByRole("listitem").length, 6);
    w.cleanup();
    const partial = w.show(lines.join("\n"), sources.slice(0, 6));
    w.click((await partial.findByText("20개")).closest("button"));
    await w.wait(() => {
      assert.equal(w.document.activeElement.tagName, "LI");
      assert.ok(w.document.activeElement.textContent.includes("[7]"));
    });
    w.cleanup();

    const latest = w.show(
      "### 핵심\n\n수주 증가 [2].\n\n### 판단 근거\n\n- 매출 증가 [26].\n\n### 확인 조건\n\n원가 검토 [2].",
      sources,
    );
    const disclosure = (
      await latest.findByText("인용 2건 / 전체 26건")
    ).closest("button");
    assert.equal(disclosure.getAttribute("aria-expanded"), "false");
    assert.equal(latest.getAllByRole("heading", { level: 4 }).length, 3);
    w.click(latest.getByRole("link", { name: "[26]", exact: true }));
    await w.wait(() => assert.equal(w.document.activeElement.id, "economy-26"));
    assert.deepEqual(Array.from(w.scrolled.at(-1)), ["economy-26", "nearest"]);

    const viewport = latest.container;
    viewport.setAttribute("data-scroll-viewport", "");
    Object.defineProperties(viewport, {
      scrollHeight: { value: 1200 },
      clientHeight: { value: 400 },
    });
    Object.defineProperty(w.document.documentElement, "clientHeight", {
      value: 700,
    });
    viewport.getBoundingClientRect = () => ({ top: 100, bottom: 500 });
    const motions = [];
    viewport.scrollBy = ({ top }) => {
      motions.push(top);
    };
    const item = w.document.getElementById("economy-26");
    const link = latest.getByRole("link", { name: "[26]", exact: true });
    const checkScroll = async (top, bottom, expected) => {
      item.getBoundingClientRect = () => ({ top, bottom });
      w.focusCitation(link);
      w.click(link);
      await w.wait(() => assert.equal(w.document.activeElement.id, item.id));
      assert.deepEqual(motions.splice(0), expected);
    };
    await checkScroll(120, 450, []);
    await checkScroll(90, 160, [-26]);
    await checkScroll(460, 520, [36]);
    const additional = latest
      .getByText("추가 자료", { exact: true })
      .closest("button");
    assert.equal(additional.getAttribute("aria-expanded"), "false");
    assert.equal(latest.queryByText("공개 자료 25", { exact: true }), null);
    const action = latest
      .getByText("원문 열기 ↗", { exact: true })
      .closest("button");
    assert.equal(action.getAttribute("role"), "link");
    const labels = action
      .getAttribute("aria-labelledby")
      .split(" ")
      .map((id) => w.document.getElementById(id).textContent.trim());
    assert.deepEqual(labels, ["원문 열기 ↗", "[2]", "공개 자료 2"]);
    w.click(action);
    await w.wait(() =>
      assert.deepEqual(Array.from(w.opened), [
        "https://example.org/observation",
      ]),
    );
    w.click(additional);
    assert.ok(latest.getByText("공개 자료 25", { exact: true }));
  } finally {
    bundle.close();
  }
});
