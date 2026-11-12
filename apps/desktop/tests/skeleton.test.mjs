import { strict as assert } from "node:assert";
import { existsSync, readFileSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { test } from "node:test";
import { desktopRoot } from "./paths.mjs";

const srcFolders = [
  "app",
  "features",
  "shared/ds",
  "shared/ipc",
  "shared/platform",
  "shared/lib",
];

const tauriDir = join(desktopRoot, "src-tauri");
const tauriConfig = JSON.parse(
  readFileSync(join(tauriDir, "tauri.conf.json"), "utf8"),
);

test("src has the folders of architecture chapter 5", () => {
  const missing = srcFolders.filter((folder) => {
    const path = join(desktopRoot, "src", folder);
    return !(existsSync(path) && statSync(path).isDirectory());
  });
  assert.deepEqual(missing, []);
});

test("the React entry point is src/app/main.tsx", () => {
  const html = readFileSync(join(desktopRoot, "index.html"), "utf8");
  assert.match(
    html,
    /<script type="module" src="\/src\/app\/main\.tsx"><\/script>/,
  );
});

test("frontendDist points at the Vite output folder, not the placeholder", () => {
  assert.equal(
    resolve(tauriDir, tauriConfig.build.frontendDist),
    join(desktopRoot, "dist"),
  );
  assert.equal(existsSync(join(tauriDir, "empty-frontend")), false);
});

test("after the vite build step of just check, dist holds the page Tauri embeds", () => {
  const built = readFileSync(join(desktopRoot, "dist", "index.html"), "utf8");
  assert.match(built, /<div id="root"><\/div>/);
  assert.match(
    built,
    /<script type="module" crossorigin src="\/assets\/[^"]+\.js"><\/script>/,
  );
});

test("tauri build runs the Vite build first", () => {
  assert.equal(tauriConfig.build.beforeBuildCommand, "pnpm exec vite build");
});
