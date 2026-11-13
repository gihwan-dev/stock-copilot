import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseLockfile } from "../ts-deps/rules.mjs";

export const nodeFixture = (manifest, source) => {
  const boundary = source.indexOf("\n---\n");
  if (boundary < 0) throw new Error("pnpm 도구 잠금 문서가 없다");
  const version = manifest.devDependencies.prettier;
  const entries = parseLockfile(source).entries;
  const importer = entries.find(
    (entry) =>
      entry.path.join("/") === "importers/./devDependencies/prettier/version",
  );
  const value = (path) =>
    entries.find(
      (entry) => entry.doc === importer?.doc && entry.path.join("/") === path,
    )?.value;
  const key = "prettier@" + version;
  const integrity = /^\{integrity: (sha512-[A-Za-z0-9+/]+={0,2})\}$/u.exec(
    value("packages/" + key + "/resolution") ?? "",
  )?.[1];
  if (
    importer?.value !== version ||
    !integrity ||
    value("packages/" + key + "/hasBin") !== "true" ||
    value("snapshots/" + key) !== "{}" ||
    value("lockfileVersion") === undefined
  )
    throw new Error(
      "prettier 매니페스트·잠금·실행 파일·독립 스냅숏이 맞지 않는다",
    );
  return {
    managerLock: source.slice(0, boundary + 1),
    manifest: {
      name: "probe",
      private: true,
      packageManager: manifest.packageManager,
      devDependencies: { prettier: version },
    },
    lock: {
      lockfileVersion: value("lockfileVersion"),
      settings: Object.fromEntries(
        entries
          .filter(
            (entry) =>
              entry.doc === importer.doc &&
              entry.path.length === 2 &&
              entry.path[0] === "settings",
          )
          .map((entry) => [entry.path[1], JSON.parse(entry.value)]),
      ),
      importers: {
        ".": { devDependencies: { prettier: { specifier: version, version } } },
      },
      packages: { [key]: { resolution: { integrity }, hasBin: true } },
      snapshots: { [key]: {} },
    },
  };
};
const yaml = (object, depth = 0) =>
  Object.entries(object)
    .map(([key, value]) => {
      const prefix = "  ".repeat(depth) + JSON.stringify(key) + ":";
      return value !== null &&
        typeof value === "object" &&
        Object.keys(value).length > 0
        ? prefix + "\n" + yaml(value, depth + 1)
        : prefix + " " + JSON.stringify(value) + "\n";
    })
    .join("");
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
  const fixture = nodeFixture(
    JSON.parse(readFileSync(join(root, "package.json"), "utf8")),
    readFileSync(join(root, "pnpm-lock.yaml"), "utf8"),
  );
  writeFileSync(
    join(process.argv[2], "package.json"),
    JSON.stringify(fixture.manifest, undefined, 2) + "\n",
  );
  writeFileSync(
    join(process.argv[2], "pnpm-lock.yaml"),
    fixture.managerLock + "---\n" + yaml(fixture.lock),
  );
}
