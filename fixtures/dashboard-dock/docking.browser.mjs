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
const groupSelector = "[data-tabset][data-tabset-id]";
const floatingSelector = "[data-floating-tabset]";
const tab = (page, section) =>
  page.locator(`[role="tab"][data-tab-id="${section}"]`);
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
    const right = item.querySelector('[data-floating-resize-handle="right"]');
    const bottom = item.querySelector('[data-floating-resize-handle="bottom"]');
    const doc = item.ownerDocument;
    return [
      right.contains(
        doc.elementFromPoint(rect.right - 1, rect.y + rect.height / 2),
      ),
      bottom.contains(
        doc.elementFromPoint(rect.x + rect.width / 2, rect.bottom - 1),
      ),
    ];
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
    .locator(`${workspace} [role=tab][data-tab-id]`)
    .evaluateAll((tabs) => tabs.map((t) => t.dataset.tabId).sort());
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
  return metrics.table;
};

test("도킹 작업영역과 보유 종목 표가 프레임을 채우고 각 섹션에 접근한다", async () => {
  await withAuditServer(async (target) => {
    for (const size of viewportCases) {
      const { page, context } = await open(target, size);
      try {
        await ready(page);
        await checkSections(page);
        await checkTiling(page);
        await checkPositions(page);
        for (const section of sections) {
          const item = tab(page, section);
          await item.focus();
          await item.press("Enter");
          assert.equal(
            await item.getAttribute("aria-selected"),
            "true",
            section,
          );
          assert.ok(
            await pane(page, section).getByRole("tabpanel").isVisible(),
            section,
          );
        }
      } finally {
        await context.close();
      }
    }
  });
});

const controls = async (page, section) => {
  await page.getByRole("dialog").waitFor({ state: "hidden" });
  const sectionTab = tab(page, section);
  await sectionTab.press("Enter");
  const title = await sectionTab.innerText();
  await pane(page, section)
    .getByRole("button", { name: title + " 배치", exact: true })
    .press("Enter");
  const dialog = page.getByRole("dialog");
  await dialog.waitFor().catch((error) => {
    throw new Error(`프레임 배치 메뉴를 열지 못함: ${section}`, {
      cause: error,
    });
  });
  return dialog;
};

const move = async (page, section, target, action) => {
  const id = await pane(page, target).getAttribute("data-tabset-id");
  const dialog = await controls(page, section);
  await dialog.getByRole("combobox", { name: "대상 프레임" }).selectOption(id);
  await dialog
    .getByRole("button", { name: action, exact: true })
    .press("Enter");
};

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
const closeTo = (a, b) =>
  assert.ok(Math.abs(a - b) <= 3, `geometry ${a} != ${b}`);

test("포인터와 키보드로 섹션을 묶고 분리하고 크기를 바꿔 복원한다", async () => {
  await withAuditServer(async (target) => {
    const { page, context } = await open(target, viewportCases[0]);
    try {
      await ready(page);
      const original = await pane(page, "briefing").getAttribute(
        "data-tabset-id",
      );
      const destination = await pane(page, "positions").getAttribute(
        "data-tabset-id",
      );
      assert.notEqual(original, destination);
      await drag(
        page,
        center(await tab(page, "briefing").boundingBox()),
        center(
          await pane(page, "positions")
            .locator("[data-tab-panels]")
            .boundingBox(),
        ),
      );
      await pane(page, "positions")
        .locator('[role="tab"][data-tab-id="briefing"]')
        .waitFor();
      await move(page, "briefing", "positions", "오른쪽에 놓기");
      assert.notEqual(
        await pane(page, "positions").getAttribute("data-tabset-id"),
        await pane(page, "briefing").getAttribute("data-tabset-id"),
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

      const dialog = await controls(page, "positions");
      await dialog
        .getByRole("button", { name: "독립 프레임으로 띄우기", exact: true })
        .press("Enter");
      const floating = page
        .locator(floatingSelector)
        .filter({ has: tab(page, "positions") });
      await floating.waitFor();
      await checkFloatingBounds(page, floating);
      const start = await floating.boundingBox();
      const grip = center(
        await floating
          .locator('[data-floating-resize-handle="top-left"]')
          .boundingBox(),
      );
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
      await move(page, "positions", "briefing", "탭으로 묶기");
      assert.equal(await page.locator(floatingSelector).count(), 0);
      assert.equal(
        await pane(page, "positions").getAttribute("data-tabset-id"),
        await pane(page, "briefing").getAttribute("data-tabset-id"),
      );
      const groupMenu = await controls(page, "positions");
      await groupMenu
        .getByRole("button", { name: "그룹 전체", exact: true })
        .press("Enter");
      await groupMenu
        .getByRole("button", { name: "독립 프레임으로 띄우기", exact: true })
        .press("Enter");
      const groupFloat = page
        .locator(floatingSelector)
        .filter({ has: tab(page, "positions") });
      const ids = await groupFloat
        .getByRole("tab")
        .evaluateAll((items) => items.map((item) => item.dataset.tabId).sort());
      assert.deepEqual(ids, ["briefing", "positions"]);
      const attach = await controls(page, "positions");
      await attach
        .getByRole("button", { name: "그룹 전체", exact: true })
        .press("Enter");
      const anchor = await pane(page, "overview").getAttribute(
        "data-tabset-id",
      );
      await attach
        .getByRole("combobox", { name: "대상 프레임" })
        .selectOption(anchor);
      await attach
        .getByRole("button", { name: "아래에 놓기", exact: true })
        .press("Enter");
      assert.equal(await page.locator(floatingSelector).count(), 0);
      assert.equal(
        await pane(page, "positions").getAttribute("data-tabset-id"),
        await pane(page, "briefing").getAttribute("data-tabset-id"),
      );
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
      for (const section of sections) {
        const dialog = await controls(page, section);
        await dialog
          .getByRole("button", { name: "위젯 제거", exact: true })
          .press("Enter");
        assert.equal(await tab(page, section).count(), 0);
      }
      await page.reload();
      await ready(page);
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
      const reset = await catalog(page);
      await reset
        .getByRole("button", { name: "기본 배치 복원", exact: true })
        .press("Enter");
      await checkSections(page);
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
          .locator('[data-dock-divider="horizontal"]')
          .first()
          .boundingBox();
        const vertical = await page
          .locator('[data-dock-divider="vertical"]')
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
            .querySelector('[data-dock-divider="horizontal"]')
            .getBoundingClientRect().width;
          const corner = globalThis.getComputedStyle(
            root.querySelector("[data-tabset-id]"),
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
