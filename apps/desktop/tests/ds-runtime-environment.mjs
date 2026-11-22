import { ESLint as BaseESLint } from "eslint";
import { realpathSync, symlinkSync, writeFileSync, rmSync } from "node:fs";
import { mkdtempSync as makeDirectory } from "../../../harness/ci/test-temp.mjs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { desktopRoot } from "./paths.mjs";

export const mkdtempSync = (prefix) => {
  const folder = realpathSync(makeDirectory(join(tmpdir(), basename(prefix))));
  try {
    symlinkSync(
      join(desktopRoot, "node_modules"),
      join(folder, "node_modules"),
      "dir",
    );
    writeFileSync(
      join(folder, "package.json"),
      JSON.stringify({ private: true, type: "module" }),
    );
    return folder;
  } catch (error) {
    rmSync(folder, { recursive: true, force: true });
    throw error;
  }
};

export function ESLint(options) {
  const overrides =
    options.overrideConfig === undefined
      ? []
      : Array.isArray(options.overrideConfig)
        ? options.overrideConfig
        : [options.overrideConfig];
  return new BaseESLint({
    ...options,
    overrideConfig: [
      ...overrides,
      {
        languageOptions: {
          parserOptions: { disallowAutomaticSingleRunInference: true },
        },
      },
    ],
  });
}
