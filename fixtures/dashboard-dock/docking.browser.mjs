import assert from "node:assert/strict";
import test from "node:test";
import { withAuditServer } from "../../harness/screen-audit/run.mjs";

const sections = [
  "overview",
  "positions",
  "briefing",
  "signals",
  "allocation",
  "holdings",
  "candidates",
  "economy",
];
const workspace = "[data-dock-workspace]";
const groupSelector = ".flexlayout__tabset";
const floatingSelector = ".flexlayout__float_window";
const tab = (page, section) =>
  page.locator(`#flexlayout-tabbutton-${section}[role=tab]`);
const panel = (page, section) =>
  page.locator(`#flexlayout-tab-${section}[role=tabpanel]`);
const pane = (page, section) =>
  page.locator(groupSelector).filter({ has: tab(page, section) });
const checkFloatingBounds = async (page, floating) => {
  const frame = await floating.boundingBox();
  const root = await page.locator(workspace).boundingBox();
  assert.ok(frame.y >= root.y - 1 && frame.x >= root.x - 1);
  assert.ok(frame.y + frame.height <= root.y + root.height + 1);
  assert.ok(frame.x + frame.width <= root.x + root.width + 1);
  const reach = await floating.evaluate((item) => {
    const rect = item.getBoundingClientRect();
    const doc = item.ownerDocument;
    const edges = [
      [rect.right - 1, rect.y + rect.height / 2, "ew-resize"],
      [rect.x + rect.width / 2, rect.bottom - 1, "ns-resize"],
    ];
    return edges.map(([x, y, cursor]) => {
      const handle = doc.elementFromPoint(x, y);
      return (
        item.contains(handle) && getComputedStyle(handle).cursor === cursor
      );
    });
  });
  assert.deepEqual(
    reach,
    [true, true],
    "부동 프레임 우측과 하단 크기 조절 손잡이",
  );
};
const viewportCases = [
  [2560, 1440, "light"],
  [1920, 1080, "light"],
  [1440, 900, "light"],
  [960, 640, "light"],
  [1440, 900, "dark"],
];

const open = async (
  target,
  [width, height, colorScheme],
  scenario = "home-ready",
) => {
  const context = await target.browser.newContext({
    viewport: { width, height },
    colorScheme,
    locale: "ko-KR",
    reducedMotion: "reduce",
  });
  const page = await context.newPage();
  page.setDefaultTimeout(5000);
  await page.goto(`${target.url}?s=${scenario}`);
  await page.locator(workspace).waitFor();
  return { page, context };
};

const ready = async (page) => {
  await page.waitForFunction(() => globalThis.auditState !== undefined);
  assert.equal(await page.evaluate(() => globalThis.auditState), "ready");
  await page.evaluate(() => globalThis.document.fonts.ready);
};

const checkTiling = async (page) => {
  const geometry = await page
    .locator(workspace)
    .evaluate((root, groupSelector) => {
      const r = root.getBoundingClientRect();
      const groups = [...root.querySelectorAll(groupSelector)].map((n) =>
        n.getBoundingClientRect(),
      );
      const uncovered = [];
      for (const h of [0.05, 0.48, 0.95]) {
        for (const w of [0.06, 0.46, 0.92]) {
          const x = r.left + w * r.width,
            y = r.top + h * r.height;
          if (
            !groups.some(
              (g) =>
                g.left - 4 <= x &&
                x <= g.right + 4 &&
                g.top - 4 <= y &&
                y <= g.bottom + 4,
            )
          )
            uncovered.push([x, y]);
        }
      }
      const doc = globalThis.document.documentElement;
      return {
        uncovered,
        coverage:
          groups.reduce((total, g) => total + g.width * g.height, 0) /
          (r.width * r.height),
        bottomGap: globalThis.innerHeight - r.bottom,
        overflow: doc.scrollWidth - doc.clientWidth,
        height: r.height,
      };
    }, groupSelector);
  assert.deepEqual(
    geometry.uncovered,
    [],
    "작업영역 내부에 도킹 그룹이 덮지 않는 구역이 있음",
  );
  assert.ok(
    Math.abs(geometry.bottomGap) <= 4,
    `작업영역 아래 ${geometry.bottomGap}px`,
  );
  assert.ok(geometry.height > 300 && geometry.overflow <= 1);
  assert.ok(
    geometry.coverage >= 0.97 && geometry.coverage <= 1.001,
    `도킹 면적 비율 ${geometry.coverage}`,
  );
};

const checkSections = async (page) => {
  const ids = await page
    .locator(`${workspace} [role=tab]`)
    .evaluateAll((tabs) =>
      tabs.map((t) => t.id.replace("flexlayout-tabbutton-", "")).sort(),
    );
  assert.deepEqual(ids, [...sections].sort(), "섹션 누락 또는 중복");
};

const checkPositions = async (page) => {
  await tab(page, "positions").click();
  const panel = page.locator('[data-dock-content="positions"]');
  const table = panel.getByRole("table");
  await table
    .getByRole("button", { name: "예시바이오", exact: true })
    .waitFor();
  const metrics = await panel.evaluate((el) => {
    const table = el.querySelector("table");
    const content = el.getBoundingClientRect(),
      sheet = table.getBoundingClientRect();
    const outerCard = table.closest("section");
    const style = globalThis.getComputedStyle(outerCard);
    return {
      free: content.bottom - sheet.bottom,
      table: sheet.height,
      heading: table.tHead.getBoundingClientRect().height,
      border: parseFloat(style.borderTopWidth),
      area: content.height,
    };
  });
  assert.ok(
    Math.abs(metrics.free) <= 20,
    `보유 종목 표 밑 여백 ${metrics.free}px, 본문 ${metrics.area}px 중 표 ${metrics.table}px`,
  );
  assert.ok(
    metrics.heading > 0 && metrics.heading <= 40,
    `보유 종목 열 헤더 높이 ${metrics.heading}px`,
  );
  assert.equal(metrics.border, 0, "프레임 안 카드 테두리");
  const last = table.getByRole("row").last();
  await last.scrollIntoViewIfNeeded();
  const rowBox = await last.boundingBox();
  const paneBox = await panel.boundingBox();
  assert.ok(
    rowBox.y >= paneBox.y - 1 &&
      rowBox.y + rowBox.height <= paneBox.y + paneBox.height + 1,
    "보유 종목 마지막 행이 프레임 안에 보인다",
  );
  assert.ok(
    await last.evaluate((row) => {
      const r = row.getBoundingClientRect();
      return row.contains(
        document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2),
      );
    }),
    "보유 종목 마지막 행을 포인터로 가리킬 수 있다",
  );
  return metrics;
};

test("도킹 작업영역과 보유 종목 표가 프레임을 채우고 각 섹션에 접근한다", async (t) => {
  await withAuditServer(async (target) => {
    for (const size of viewportCases) {
      const { page, context } = await open(target, size);
      try {
        await ready(page);
        await checkSections(page);
        await checkTiling(page);
        const metrics = await checkPositions(page);
        t.diagnostic(
          `${size.join(" ")} 보유 종목 표 ${metrics.table.toFixed(1)}px, 아래 여백 ${metrics.free.toFixed(1)}px`,
        );
        for (const section of sections) {
          const item = tab(page, section);
          await item.focus();
          await item.press("Enter");
          assert.equal(
            await item.getAttribute("aria-selected"),
            "true",
            section,
          );
          assert.ok(await panel(page, section).isVisible(), section);
        }
      } finally {
        await context.close();
      }
    }
  });
});

const menuAction = async (page, source, action, position) => {
  await source.click({ button: "right", position });
  await page
    .getByRole("menuitem", { name: action, exact: true })
    .press("Enter");
};
const dockAt = async (source, target, side) => {
  const box = await target.boundingBox();
  const offset = {
    center: [0.5, 0.5],
    right: [0.94, 0.5],
    bottom: [0.5, 0.92],
  }[side];
  await source.dragTo(target, {
    steps: 18,
    targetPosition: { x: box.width * offset[0], y: box.height * offset[1] },
  });
};
const groupPath = (page, section) =>
  pane(page, section).getAttribute("data-layout-path");

const catalog = async (page) => {
  await page
    .getByRole("button", { name: "위젯 추가", exact: true })
    .first()
    .press("Enter");
  return page.getByRole("dialog");
};

const drag = async (page, from, to) => {
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y, { steps: 18 });
  await page.mouse.up();
};

const center = (rect) => ({
  x: rect.x + rect.width / 2,
  y: rect.y + rect.height / 2,
});
const moveFloatAside = async (page, floating) => {
  const frame = await floating.boundingBox();
  const root = await page.locator(workspace).boundingBox();
  const handle = center(
    await floating.locator(".flexlayout__float_window_header").boundingBox(),
  );
  const dx = root.x + root.width - frame.x - frame.width - 16;
  await drag(page, handle, { x: handle.x + dx, y: handle.y });
  const moved = await floating.boundingBox();
  closeTo(moved.x, frame.x + dx);
};
const closeTo = (a, b) =>
  assert.ok(Math.abs(a - b) <= 3, `geometry ${a} != ${b}`);

test("포인터와 키보드로 섹션을 묶고 분리하고 크기를 바꿔 복원한다", async () => {
  await withAuditServer(async (target) => {
    const { page, context } = await open(target, viewportCases[0]);
    try {
      await ready(page);
      assert.notEqual(
        await groupPath(page, "briefing"),
        await groupPath(page, "positions"),
      );
      await dockAt(tab(page, "briefing"), panel(page, "positions"), "center");
      await pane(page, "positions")
        .locator("#flexlayout-tabbutton-briefing")
        .waitFor();
      await tab(page, "positions").click();
      await dockAt(tab(page, "briefing"), panel(page, "positions"), "right");
      assert.notEqual(
        await groupPath(page, "positions"),
        await groupPath(page, "briefing"),
      );
      const before = await pane(page, "positions").boundingBox();
      const edge = before.x + before.width;
      const splitters = await page
        .locator(workspace)
        .getByRole("separator")
        .all();
      const coords = await Promise.all(splitters.map((s) => s.boundingBox()));
      const divider = coords.find(
        (r) =>
          r &&
          Math.abs(center(r).x - edge) < 10 &&
          center(r).y >= before.y &&
          center(r).y <= before.y + before.height,
      );
      assert.ok(divider, "두 프레임의 공용 세로 분할선");
      const control = splitters[coords.indexOf(divider)];
      await control.press("ArrowRight");
      const keyboardSize = await pane(page, "positions").boundingBox();
      assert.ok(keyboardSize.width > before.width, "분할선을 키보드로 넓힌다");
      await control.press("ArrowLeft");
      const restoredSize = await pane(page, "positions").boundingBox();
      assert.ok(Math.abs(restoredSize.width - before.width) < 2);
      const from = center(await control.boundingBox());
      await drag(page, from, { x: from.x + 72, y: from.y });
      const resized = await pane(page, "positions").boundingBox();
      assert.ok(resized.width > before.width + 20);
      await checkPositions(page);

      await menuAction(page, tab(page, "positions"), "독립 프레임으로 띄우기");
      const floating = page
        .locator(floatingSelector)
        .filter({ has: tab(page, "positions") });
      await floating.waitFor();
      await checkFloatingBounds(page, floating);
      const start = await floating.boundingBox();
      const grip = { x: start.x + 2, y: start.y + 2 };
      await drag(page, grip, { x: grip.x + 50, y: grip.y + 72 });
      const after = await floating.boundingBox();
      assert.ok(
        after.height < start.height - 30 && after.width < start.width - 20,
        `부동 크기 조절 전 ${JSON.stringify(start)}, 시작점 ${JSON.stringify(grip)}, 이후 ${JSON.stringify(after)}`,
      );
      await checkPositions(page);

      await page.reload();
      await ready(page);
      const restored = await floating.boundingBox();
      assert.ok(restored);
      await checkFloatingBounds(page, floating);
      for (const key of ["x", "y", "width", "height"])
        closeTo(restored[key], after[key]);
      await moveFloatAside(page, floating);
      await dockAt(tab(page, "positions"), panel(page, "briefing"), "center");
      await floating.waitFor({ state: "detached" });
      assert.equal(
        await groupPath(page, "positions"),
        await groupPath(page, "briefing"),
      );
      const strip = pane(page, "positions").locator(
        '[data-layout-path$="/tabstrip"]',
      );
      const emptySpace = await strip.evaluate((el) => {
        const r = el.getBoundingClientRect();
        for (let x = r.width / 2; x < r.width; x += 4) {
          const hit = document.elementFromPoint(
            r.left + x,
            r.top + r.height / 2,
          );
          if (el.contains(hit) && !hit.closest("[role=tab],button"))
            return { x, y: r.height / 2 };
        }
        throw new Error("그룹 탭 사이의 빈 영역");
      });
      await menuAction(
        page,
        strip,
        "그룹을 독립 프레임으로 띄우기",
        emptySpace,
      );
      await floating.waitFor();
      const ids = await floating
        .getByRole("tab")
        .evaluateAll((tabs) =>
          tabs.map((t) => t.id.replace("flexlayout-tabbutton-", "")).sort(),
        );
      assert.deepEqual(ids, ["briefing", "positions"]);
      const overview = panel(page, "overview");
      await moveFloatAside(page, floating);
      await dockAt(floating.getByLabel("프레임에 고정"), overview, "bottom");
      await floating.waitFor({ state: "detached" });
      assert.equal(
        await groupPath(page, "positions"),
        await groupPath(page, "briefing"),
      );
      assert.notEqual(
        await groupPath(page, "positions"),
        await groupPath(page, "overview"),
      );
      const anchored = await pane(page, "positions").boundingBox();
      const above = await pane(page, "overview").boundingBox();
      assert.ok(anchored.y >= above.y + above.height - 1);
      await checkSections(page);
      await checkTiling(page);
    } finally {
      await context.close();
    }
  });
});

test("모든 위젯을 제거한 빈 배치를 저장하고 카탈로그에서 다시 연다", async () => {
  await withAuditServer(async (target) => {
    const { page, context } = await open(target, viewportCases[2]);
    try {
      await ready(page);
      const defaultGroups = await page.locator(groupSelector).count();
      const names = new Map();
      for (const section of sections) {
        names.set(section, await tab(page, section).getAttribute("aria-label"));
        await tab(page, section).press("Control+Delete");
        await tab(page, section).waitFor({ state: "detached" });
        assert.equal(await tab(page, section).count(), 0);
      }
      await page.reload();
      await page.getByText("표시할 위젯이 없어요", { exact: true }).waitFor();
      assert.equal(await page.locator(workspace).getByRole("tab").count(), 0);
      const add = await catalog(page);
      await add.getByRole("button", { name: /보유 종목/u }).press("Enter");
      await tab(page, "positions").waitFor();
      assert.equal(await page.locator(workspace).getByRole("tab").count(), 1);
      await checkPositions(page);
      const check = await catalog(page);
      assert.ok(
        await check.getByRole("button", { name: /보유 종목/u }).isDisabled(),
        "열린 위젯 중복 금지",
      );
      await page.keyboard.press("Escape");
      await page.getByRole("button", { name: "설정", exact: true }).click();
      await page.getByRole("button", { name: "대시보드", exact: true }).click();
      await tab(page, "positions").waitFor();
      assert.equal(await page.locator(workspace).getByRole("tab").count(), 1);
      for (const [section, name] of names) {
        if (section === "positions") continue;
        const addMore = await catalog(page);
        await addMore.getByRole("button", { name, exact: true }).press("Enter");
        await tab(page, section).waitFor({ state: "attached" });
      }
      await checkSections(page);
      assert.equal(await page.locator(groupSelector).count(), 1);
      const reset = await catalog(page);
      await reset
        .getByRole("button", { name: "기본 배치 복원", exact: true })
        .press("Enter");
      await checkSections(page);
      assert.equal(await page.locator(groupSelector).count(), defaultGroups);
      await checkTiling(page);
    } finally {
      await context.close();
    }
  });
});

test("화면 모양 설정을 대시보드에 적용하고 다시 열 때 유지한다", async () => {
  await withAuditServer(async (target) => {
    const { page, context } = await open(target, viewportCases[0]);
    try {
      await ready(page);
      await page.getByRole("button", { name: "설정", exact: true }).click();
      for (const [value, expected] of [
        ["0", 1],
        ["4", 4],
        ["8", 8],
      ]) {
        await page
          .getByRole("group", { name: "위젯 간격", exact: true })
          .getByRole("button", { name: value + " 픽셀", exact: true })
          .press("Enter");
        await page
          .getByRole("button", { name: "대시보드", exact: true })
          .click();
        await tab(page, "overview").waitFor();
        const horizontal = await page
          .locator(`${workspace} [role=separator][aria-orientation=vertical]`)
          .first()
          .boundingBox();
        const vertical = await page
          .locator(`${workspace} [role=separator][aria-orientation=horizontal]`)
          .first()
          .boundingBox();
        assert.equal(horizontal.width, expected);
        assert.equal(vertical.height, expected);
        await page.getByRole("button", { name: "설정", exact: true }).click();
      }
      for (const [name, value] of [
        ["위젯 간격", "12"],
        ["안쪽 여백", "16"],
        ["카드 모서리", "8"],
      ]) {
        await page
          .getByRole("group", { name, exact: true })
          .getByRole("button", { name: value + " 픽셀", exact: true })
          .press("Enter");
      }
      const appearance = () =>
        page.locator(workspace).evaluate((root) => {
          const gap = root
            .querySelector("[role=separator][aria-orientation=vertical]")
            .getBoundingClientRect().width;
          const corner = globalThis.getComputedStyle(
            root.querySelector(".flexlayout__tabset"),
          ).borderTopLeftRadius;
          const padding = globalThis.getComputedStyle(
            root.querySelector('[data-widget-frame="overview"]'),
          ).paddingLeft;
          return { gap, corner, padding };
        });
      await page.getByRole("button", { name: "대시보드", exact: true }).click();
      await tab(page, "overview").waitFor();
      assert.deepEqual(await appearance(), {
        gap: 12,
        corner: "8px",
        padding: "16px",
      });
      await page.reload();
      await ready(page);
      await tab(page, "overview").waitFor();
      assert.deepEqual(await appearance(), {
        gap: 12,
        corner: "8px",
        padding: "16px",
      });
    } finally {
      await context.close();
    }
  });
});
