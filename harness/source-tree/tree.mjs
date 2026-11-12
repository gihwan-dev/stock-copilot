import {
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

export const here = dirname(fileURLToPath(import.meta.url));
export const appSource = join(here, "..", "..", "apps", "desktop", "src");

const entriesOf = (folder) =>
  readdirSync(folder).map((name) => join(folder, name));

export const linksUnder = (root) => {
  const visit = (folder) =>
    entriesOf(folder).flatMap((path) => {
      const stat = lstatSync(path);
      return stat.isSymbolicLink()
        ? [relative(root, path).split(sep).join("/")]
        : stat.isDirectory()
          ? visit(path)
          : [];
    });
  return lstatSync(root).isSymbolicLink() ? ["."] : visit(root);
};

const fixtures = {
  clean: { files: { "features/a/model/value.ts": "" }, links: {} },
  "file-link": {
    files: { "features/b/model/value.ts": "" },
    links: { "features/a/model/value.ts": "../../b/model/value.ts" },
  },
  "folder-link": {
    files: { "features/b/model/value.ts": "" },
    links: { "features/a/model": "../b/model" },
  },
  "nested-folder-link": {
    files: { "features/b/model/value.ts": "" },
    links: { "features/a/ui/deep/inner": "../../../b/model" },
  },
};

export const fixtureExpectations = {
  clean: [],
  "file-link": ["features/a/model/value.ts"],
  "folder-link": ["features/a/model"],
  "nested-folder-link": ["features/a/ui/deep/inner"],
};

export const withFixture = (name, action) => {
  const root = mkdtempSync(join(tmpdir(), "source-tree-"));
  const { files, links } = fixtures[name];
  Object.entries(files).forEach(([path, text]) => {
    mkdirSync(dirname(join(root, path)), { recursive: true });
    writeFileSync(join(root, path), text);
  });
  Object.entries(links).forEach(([path, target]) => {
    mkdirSync(dirname(join(root, path)), { recursive: true });
    symlinkSync(target, join(root, path));
  });
  const result = action(root);
  rmSync(root, { recursive: true, force: true });
  return result;
};

export const fixtureNames = Object.keys(fixtures);
