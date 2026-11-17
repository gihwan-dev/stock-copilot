import { strict as assert } from "node:assert";
import { test } from "node:test";
import { createContext, runInContext } from "node:vm";
import {
  readFileSync,
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  rmSync,
  cpSync,
} from "node:fs";
import { join } from "node:path";
import { request } from "node:http";
import { captureStory } from "../../../harness/visual/capture.mjs";
import { staticServer } from "../../../harness/visual/server.mjs";
import {
  workflowFindings,
  repositoryFindings,
  readPolicy,
} from "../../../harness/ci/workflow-policy/check.mjs";
import { readTools, installActionSpec } from "../../../harness/ci/tools.mjs";
import { repoRoot } from "./paths.mjs";

const fixture = ({
  font = "custom",
  input = "fixed",
  pageError = "absent",
  fontList = "complete",
} = {}) => {
  const calls = [];
  const fonts = [
    { family: "IBM Plex Sans KR", status: "loaded" },
    { family: "IBM Plex Mono", status: "loaded" },
  ];
  if (fontList === "missingMono") fonts.splice(1, 1);
  if (fontList === "missingSans") fonts.splice(0, 1);
  fonts.ready = Promise.resolve();
  fonts.load = async (value) => {
    calls.push(["load", value]);
  };
  const realm = createContext({ document: { fonts } });
  const session = {
    send: async (command, argument) => {
      calls.push([command, argument]);
      if (command === "DOM.getDocument") return { root: { nodeId: 41 } };
      if (command === "DOM.querySelectorAll") return { nodeIds: [101, 202] };
      if (command === "CSS.getPlatformFontsForNode")
        return {
          fonts: [
            {
              glyphCount: font === "unused" ? 0 : 7,
              isCustomFont: font === "custom",
            },
          ],
        };
      return {};
    },
    detach: async () => {
      calls.push(["detach"]);
    },
  };
  let installedTime;
  const page = {
    on: (name, handler) => {
      calls.push(["on", name]);
      if (pageError === "present") handler(new Error("page failed"));
    },
    clock: {
      install: async (value) => {
        calls.push(["install", value.time.toISOString()]);
        installedTime = value.time.getTime() + 1;
      },
      pauseAt: async (time) => {
        calls.push(["pauseAt", time.toISOString()]);
        if (time.getTime() < installedTime)
          throw new Error("Cannot fast-forward to the past");
        realm.Date = {
          now: () => (input === "moving" ? 1 : time.getTime()),
        };
      },
    },
    goto: async (...args) => {
      calls.push(["goto", ...args]);
    },
    locator: (selector) => ({
      waitFor: async () => {
        calls.push(["ready", selector]);
      },
    }),
    addStyleTag: async (style) => {
      calls.push(["style", style.content]);
    },
    evaluate: async (fn) => runInContext(`(${fn.toString()})()`, realm),
    screenshot: async (options) => {
      calls.push(["screenshot", options]);
      return Buffer.from([137, 80, 78, 71, 49]);
    },
  };
  const context = {
    route: async (pattern, handler) => {
      calls.push(["route", pattern]);
      for (const url of [
        "http://127.0.0.1:4321/a.js",
        "https://example.invalid/a.js",
      ])
        await handler({
          request: () => ({ url: () => url }),
          continue: async () => calls.push(["continue", url]),
          abort: async () => calls.push(["abort", url]),
        });
    },
    addInitScript: async (fn, value) => {
      runInContext(`(${fn.toString()})(${JSON.stringify(value)})`, realm);
      if (input === "random") runInContext("Math.random = () => 0.9", realm);
    },
    newPage: async () => page,
    newCDPSession: async () => session,
    close: async () => {
      calls.push(["close"]);
    },
  };
  const browser = {
    newContext: async (options) => {
      calls.push(["context", options]);
      return context;
    },
  };
  return { calls, browser, realm };
};
const story = {
  origin: "http://127.0.0.1:4321",
  entry: { id: "catalog-money--dark-0" },
  component: "Money",
  theme: "dark",
};

test("DS15 normal capture applies inputs and checks actual fonts before its screenshot", async () => {
  const { calls, browser, realm } = fixture();
  const result = await captureStory(browser, story);
  assert.deepEqual(result, Buffer.from([137, 80, 78, 71, 49]));
  assert.deepEqual(
    calls.find(([name]) => name === "context"),
    [
      "context",
      {
        viewport: { width: 960, height: 640 },
        deviceScaleFactor: 1,
        locale: "ko-KR",
        timezoneId: "Asia/Seoul",
        colorScheme: "dark",
        reducedMotion: "reduce",
        serviceWorkers: "block",
      },
    ],
  );
  assert.equal(runInContext("Math.random()", realm), 0.3141592653589793);
  assert.deepEqual(
    calls.filter(([name]) => ["install", "pauseAt"].includes(name)),
    [
      ["install", "1970-01-01T00:00:00.000Z"],
      ["pauseAt", "2026-01-02T03:04:05.000Z"],
    ],
  );
  assert.deepEqual(
    calls.find(([name]) => name === "goto"),
    [
      "goto",
      "http://127.0.0.1:4321/iframe.html?id=catalog-money--dark-0&viewMode=story",
      { waitUntil: "networkidle" },
    ],
  );
  assert.deepEqual(
    calls.filter(([name]) => name === "continue" || name === "abort"),
    [
      ["continue", "http://127.0.0.1:4321/a.js"],
      ["abort", "https://example.invalid/a.js"],
    ],
  );
  assert.deepEqual(
    calls.find(([name]) => name === "ready"),
    ["ready", '[data-catalog-component="Money"][data-catalog-theme="dark"]'],
  );
  assert.equal(calls.filter(([name]) => name === "detach").length, 1);

  assert.deepEqual(
    calls.filter(([name]) => name === "load"),
    [
      ["load", '400 16px "IBM Plex Sans KR"'],
      ["load", '400 16px "IBM Plex Mono"'],
    ],
  );
  const css = calls.find(([name]) => name === "style")[1];
  assert.match(css, /animation: none/);
  assert.match(css, /transition: none/);
  assert.match(css, /caret-color: transparent/);
  assert.doesNotMatch(css, /font-family/);
  assert.deepEqual(
    calls.filter(([name]) => name === "CSS.getPlatformFontsForNode"),
    [
      ["CSS.getPlatformFontsForNode", { nodeId: 101 }],
      ["CSS.getPlatformFontsForNode", { nodeId: 202 }],
    ],
  );
  assert.deepEqual(
    calls.find(([name]) => name === "screenshot"),
    ["screenshot", { animations: "disabled", caret: "hide" }],
  );
  assert.ok(
    calls.findIndex(([name]) => name === "CSS.getPlatformFontsForNode") <
      calls.findIndex(([name]) => name === "screenshot"),
  );
  assert.deepEqual(calls.at(-1), ["close"]);
});

test("DS15 normal capture rejects system font fallback before capturing", async () => {
  const { calls, browser } = fixture({ font: "system" });
  await assert.rejects(captureStory(browser, story), /시스템 글꼴 사용/);
  assert.equal(
    calls.some(([name]) => name === "screenshot"),
    false,
  );
  assert.deepEqual(calls.at(-1), ["close"]);
});

test("DS15 normal capture rejects a clock that did not freeze", async () => {
  const { calls, browser } = fixture({ input: "moving" });
  await assert.rejects(captureStory(browser, story), /시계·난수 고정 실패/);
  assert.equal(
    calls.some(([name]) => name === "screenshot"),
    false,
  );
  assert.deepEqual(calls.at(-1), ["close"]);
});

test("DS15 product stylesheet selects the bundled font without a capture override", () => {
  const css = readFileSync(
    join(repoRoot, "apps/desktop/src/shared/ds/theme.css"),
    "utf8",
  );
  assert.match(
    css,
    /@layer base\s*{\s*html\s*{\s*font-family: var\(--font-sans\);/,
  );
});

test("DS15 malformed static requests return errors while the server keeps serving", async () => {
  mkdirSync(join(repoRoot, "target"), { recursive: true });
  const folder = mkdtempSync(join(repoRoot, "target/DS-15-server-"));
  mkdirSync(join(folder, "directory"));
  writeFileSync(join(folder, "normal.html"), "normal");
  const server = staticServer(folder);
  await new Promise((done) => server.listen(0, "127.0.0.1", done));
  const get = (path) =>
    new Promise((done, reject) => {
      const req = request(
        { hostname: "127.0.0.1", port: server.address().port, path },
        (response) => {
          response.resume();
          response.on("end", () => done(response.statusCode));
        },
      );
      req.on("error", reject);
      req.end();
    });
  try {
    assert.equal(await get("/normal.html"), 200);
    assert.equal(await get("/%E0"), 400);
    assert.equal(await get("/directory"), 404);
    assert.equal(await get("/%2e%2e%2foutside.html"), 404);
    assert.equal(await get("/normal.html"), 200);
  } finally {
    await new Promise((done) => server.close(done));
    rmSync(folder, { recursive: true, force: true });
  }
});

test("DS15 Linux CI and check-full require the container comparison", () => {
  const workflow = JSON.parse(
    readFileSync(join(repoRoot, ".github/workflows/ci.yml"), "utf8"),
  );
  assert.equal(workflow.jobs.linux["runs-on"], "ubuntu-24.04-arm");
  const steps = workflow.jobs.linux.steps;
  const compare = steps.findIndex(
    (step) => step.run === "node harness/ci/visual.mjs",
  );
  assert.ok(compare > steps.findIndex((step) => step.name === "tools"));
  assert.ok(compare > steps.findIndex((step) => step.name === "advisory db"));
  const policy = readPolicy(repoRoot);
  const tools = installActionSpec(readTools(repoRoot));
  assert.deepEqual(
    workflowFindings(
      ".github/workflows/ci.yml",
      JSON.stringify(workflow, null, 2) + "\n",
      policy,
      tools,
    ),
    [],
  );
  const missing = structuredClone(workflow);
  missing.jobs.linux.steps.splice(compare, 1);
  mkdirSync(join(repoRoot, "target"), { recursive: true });
  const folder = mkdtempSync(join(repoRoot, "target/DS-15-ci-"));
  try {
    cpSync(join(repoRoot, ".github"), join(folder, ".github"), {
      recursive: true,
    });
    assert.deepEqual(repositoryFindings(folder, policy, tools), []);
    writeFileSync(
      join(folder, ".github/workflows/ci.yml"),
      JSON.stringify(missing, null, 2) + "\n",
    );
    assert.deepEqual(repositoryFindings(folder, policy, tools), [
      { path: ".github/workflows/ci.yml", rule: "expected" },
    ]);
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }

  const full = readFileSync(join(repoRoot, "justfile"), "utf8")
    .split("check-full: advisory-db-fetch check\n")[1]
    .split("\n\n")[0];
  assert.match(full, /visual-check/);
});

test("DS15 normal capture permits zero-glyph system font records", async () => {
  const { browser } = fixture({ font: "unused" });
  assert.deepEqual(
    await captureStory(browser, story),
    Buffer.from([137, 80, 78, 71, 49]),
  );
});

test("DS15 normal capture rejects a missing bundled family or page error", async () => {
  for (const options of [
    { fontList: "missingMono" },
    { fontList: "missingSans" },
    { pageError: "present" },
  ]) {
    const { browser, calls } = fixture(options);
    await assert.rejects(captureStory(browser, story), /글꼴\/페이지 실패/);
    assert.equal(
      calls.some(([name]) => name === "screenshot"),
      false,
    );
    assert.deepEqual(calls.at(-1), ["close"]);
  }
});

test("DS15 normal capture rejects a random source that did not freeze", async () => {
  const { browser, calls } = fixture({ input: "random" });
  await assert.rejects(captureStory(browser, story), /시계·난수 고정 실패/);
  assert.equal(
    calls.some(([name]) => name === "screenshot"),
    false,
  );
  assert.deepEqual(calls.at(-1), ["close"]);
});
