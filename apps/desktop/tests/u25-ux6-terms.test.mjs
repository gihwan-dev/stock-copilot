import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";
import test from "node:test";
import { repoRoot } from "./paths.mjs";
import { exceptionFolders, forbiddenTerms } from "./ux6-terms.mjs";

const screenRoots = [
  "apps/desktop/src",
  "apps/desktop/src-tauri/src",
  "crates/application/src",
];

const skippedFile =
  /\.test\.|\.stories\.|\/tests?\/|[/_]tests?\.rs$|\/testing\/|\/ipc\/bindings\.ts$/;

const literal =
  /"(?:[^"\\\n]|\\.)*"|'(?:[^'\\\n]|\\.)*'|`(?:[^`\\]|\\.)*`|>[^<>{}]*</g;

const walk = (folder) =>
  readdirSync(folder).flatMap((name) => {
    const path = join(folder, name);
    return statSync(path).isDirectory() ? walk(path) : [path];
  });

const productSource = (path, source) => {
  if (!path.endsWith(".rs")) return source;
  const testStart = source.indexOf("#[cfg(test)]");
  return testStart < 0 ? source : source.slice(0, testStart);
};

const koreanTextHits = (source) =>
  [...source.matchAll(literal)]
    .map((match) => match[0])
    .filter((text) => /[가-힣]/u.test(text))
    .flatMap((text) =>
      forbiddenTerms
        .filter((term) => text.includes(term))
        .map((term) => ({ term, text })),
    );

const screenFiles = () =>
  screenRoots
    .flatMap((root) => walk(join(repoRoot, root)))
    .map((path) => relative(repoRoot, path).split(sep).join("/"))
    .filter((path) => /\.(ts|tsx|rs)$/.test(path))
    .filter((path) => !skippedFile.test(path));

const excepted = (path) =>
  exceptionFolders.some((folder) => path.startsWith(folder));

test("UX-6 Korean strings in screen code and core screen text contain no internal terms outside the advanced status screen", () => {
  const files = screenFiles();
  assert.ok(files.length > 50, `scanned only ${files.length} files`);
  const hits = files
    .filter((path) => !excepted(path))
    .flatMap((path) =>
      koreanTextHits(
        productSource(path, readFileSync(join(repoRoot, path), "utf8")),
      ).map((hit) => `${path}: ${hit.term}: ${hit.text}`),
    );
  assert.deepEqual(hits, []);
});

test("UX-6 scanner flags each forbidden term in Korean strings, JSX text and template text", () => {
  forbiddenTerms.forEach((term) => {
    assert.equal(koreanTextHits(`"${term}이 끊겼어요"`).length, 1, term);
    assert.equal(koreanTextHits(`>${term} 확인 중<`).length, 1, term);
    assert.equal(koreanTextHits(`\`${term} 다시 시도\``).length, 1, term);
  });
  assert.deepEqual(koreanTextHits(`"잠시 뒤 다시 시도해 주세요"`), []);
  assert.deepEqual(koreanTextHits(`const sessionQueue = "ready";`), []);
});

test("UX-6 Rust test modules and the advanced status screen are the only places left out of the scan", () => {
  assert.equal(
    productSource(
      "x.rs",
      `const A: &str = "정상";\n#[cfg(test)]\nmod t { "세션"; }`,
    ),
    `const A: &str = "정상";\n`,
  );
  assert.deepEqual(exceptionFolders, ["apps/desktop/src/features/status/"]);
  exceptionFolders.forEach((folder) =>
    assert.ok(existsSync(join(repoRoot, folder)), folder),
  );
  assert.ok(excepted("apps/desktop/src/features/status/messages.ts"));
  assert.ok(!excepted("apps/desktop/src/features/portfolio/messages.ts"));
});
