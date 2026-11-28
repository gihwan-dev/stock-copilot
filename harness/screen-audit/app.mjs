import { createElement as h } from "react";
import { createRoot } from "react-dom/client";
import { App } from "../../apps/desktop/src/app/App.tsx";
import { createAppQueryClient } from "../../apps/desktop/src/app/query.ts";
import { mockCommands } from "../../apps/desktop/src/shared/ipc/testing/mockBindings.ts";
import { scenarios } from "./scenarios.mjs";

const page = globalThis;
const name = new URL(page.location.href).searchParams.get("s") ?? "";
const scenario = scenarios[name];

const pause = (ms) => new Promise((done) => setTimeout(done, ms));

const findByText = (selector, needle) =>
  [...page.document.querySelectorAll(selector)].find(
    (node) => (node.textContent ?? "").trim() === needle,
  );

const waitFor = async (probe, label) => {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    const found = probe();
    if (found !== undefined && found !== null) return found;
    await pause(10);
  }
  throw new Error(`대기 실패: ${label}`);
};

const helpers = {
  waitText: (needle) =>
    waitFor(
      () =>
        (page.document.body.textContent ?? "").includes(needle)
          ? true
          : undefined,
      needle,
    ),
  click: async (needle, selector = "button, [role=button], [role=tab], a") => {
    const target = await waitFor(() => findByText(selector, needle), needle);
    target.click();
  },
  clickFirst: async (selector) => {
    const target = await waitFor(
      () => page.document.querySelector(selector),
      selector,
    );
    target.click();
  },
  pause,
};

const start = async () => {
  if (scenario === undefined) throw new Error(`없는 장면: ${name}`);
  const setup = scenario.setup();
  mockCommands(setup.commands);
  createRoot(page.document.getElementById("root")).render(
    h(App, {
      initialPath: new URL(page.location.href).searchParams.has("hash-route")
        ? undefined
        : scenario.path,
      queryClient: createAppQueryClient(),
      sources: setup.sources,
      settingsClient: setup.settingsClient,
      setupClient: setup.setupClient,
    }),
  );
  if (scenario.act !== undefined) await scenario.act(helpers);
  await pause(150);
};

page.auditNames = Object.keys(scenarios);
page.auditPaths = Object.values(scenarios).map((item) => item.path);

(name === "" ? Promise.resolve() : start()).then(
  () => {
    page.auditState = "ready";
  },
  (error) => {
    page.auditState = `error: ${error.message}`;
  },
);
