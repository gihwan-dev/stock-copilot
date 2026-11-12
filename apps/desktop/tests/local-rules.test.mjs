import { createRequire } from "node:module";
import { join } from "node:path";
import { describe, it } from "node:test";
import { local } from "../eslint/local-plugin.mjs";
import { desktopRoot, repoRoot } from "./paths.mjs";

const requireFromDesktop = createRequire(join(desktopRoot, "package.json"));
const { RuleTester } = requireFromDesktop("eslint");
const tseslint = requireFromDesktop("typescript-eslint");

const harness = join(repoRoot, "harness", "eslint-violations");
process.chdir(harness);

RuleTester.describe = describe;
RuleTester.it = it;

const typed = new RuleTester({
  languageOptions: {
    parser: tseslint.parser,
    parserOptions: {
      projectService: true,
      tsconfigRootDir: harness,
      ecmaFeatures: { jsx: true },
    },
  },
});
const plain = new RuleTester({
  languageOptions: {
    parser: tseslint.parser,
    parserOptions: { ecmaFeatures: { jsx: true } },
  },
});

const at = (path) => join(harness, path);
const ts = at("src/features/raw/allowed.ts");
const tsx = at("src/features/raw/props-alias.tsx");
const lib = at("src/shared/lib/key-strings.ts");
const errors = (...ids) => ids.map((messageId) => ({ messageId }));

plain.run("local/no-comments", local.rules["no-comments"], {
  valid: [
    'export const a = "a";',
    'export const a = "// not a comment";',
    "export const f = (): void => {\n  // eslint-disable-next-line no-console -- 이유\n  console.info(1);\n};",
  ],
  invalid: [
    { code: "// 설명\nexport const a = 1;", errors: errors("comment") },
    { code: "/* 설명 */ export const a = 1;", errors: errors("comment") },
    { code: "/** 문서 */\nexport const a = 1;", errors: errors("comment") },
    {
      code: '/// <reference types="react" />\nexport const a = 1;',
      errors: errors("comment"),
    },
    {
      code: "#!/usr/bin/env node\nexport const a = 1;",
      errors: errors("comment"),
    },
    {
      code: "export const e = <b>{/* 설명 */}</b>;",
      filename: "x.tsx",
      errors: errors("comment"),
    },
    {
      code: "/* eslint-disable-next-line no-console -- 블록 */\nconsole.info(1);",
      errors: errors("comment"),
    },
    {
      code: "// eslint-disable-next-line no-console\nconsole.info(1);",
      errors: errors("comment"),
    },
    {
      code: "// eslint-disable-next-line no-console, no-alert -- 둘\nconsole.info(1);",
      errors: errors("comment"),
    },
    {
      code: "console.info(1); // eslint-disable-line no-console -- 이유",
      errors: errors("comment"),
    },
  ],
});

const moneyOptions = [
  {
    moneyFile: "src/shared/lib/money.ts",
    keyReaders: ["src/shared/lib/format.ts"],
  },
];
const moneyImport = 'import type { Money } from "../../shared/lib/money";\n';
const money = (body) => ({
  code: `${moneyImport}${body}`,
  filename: ts,
  options: moneyOptions,
});

typed.run("local/no-money-coercion", local.rules["no-money-coercion"], {
  valid: [
    money('export const u = (a: Money): "KRW" | "USD" => a.currency;'),
    money(
      'export const u = (a: Money | undefined): "y" | "n" => (a === undefined ? "n" : "y");',
    ),
    money("export const k = (a: Money): Money => a;"),
    {
      code: 'import { moneyKey, type Money } from "./money";\nexport const d = (a: Money): `${number}` => a[moneyKey];',
      filename: at("src/shared/lib/format.ts"),
      options: moneyOptions,
    },
    money(
      'export const n = (a: "1"): "x" => (Number(a) === 1 && `${a}` === "1" ? "x" : "x");',
    ),
  ],
  invalid: [
    {
      code: 'import * as M from "../../shared/lib/money";\nconst get = <O, K extends keyof O>(o: O, k: K): O[K] => o[k];\nconst name = "moneyKey";\nexport const c = (a: M.Money, b: M.Money): "x" => (get(a, M[name]) > get(b, M[name]) ? "x" : "x");',
      filename: ts,
      options: moneyOptions,
      errors: errors("generic", "extraction", "generic", "extraction"),
    },
    {
      ...money("export const c = (a: Money): Money => structuredClone(a);"),
      errors: errors("generic"),
    },
    {
      ...money(
        'export const c = (a: Money, b: Money): "x" => (a < b ? "x" : "x");',
      ),
      errors: errors("operator"),
    },
    {
      ...money(
        'export const c = (a: Money, b: Money): "x" => (a === b ? "x" : "x");',
      ),
      errors: errors("operator"),
    },
    {
      ...money(
        'export const c = (a: Money): "x" => (a + "원" === "" ? "x" : "x");',
      ),
      errors: errors("operator"),
    },
    {
      ...money('export const c = (a: Money): "x" => (-a === 0 ? "x" : "x");'),
      errors: errors("operator"),
    },
    {
      ...money(
        'export const c = <T extends Money>(a: T, b: T): "x" => (a >= b ? "x" : "x");',
      ),
      errors: errors("operator"),
    },
    {
      ...money(
        'export const c = (a: Money): "x" => (`${a}` === "" ? "x" : "x");',
      ),
      errors: errors("template"),
    },
    ...["Number", "String", "Boolean", "parseFloat", "JSON.stringify"].map(
      (callee) => ({
        ...money(
          `export const c = (a: Money): "x" => (${callee}(a) ? "x" : "x");`,
        ),
        errors: errors("conversion"),
      }),
    ),
    {
      ...money(
        'export const c = (a: Money): "x" => (Object.getOwnPropertySymbols(a).length === 0 ? "x" : "x");',
      ),
      errors: errors("conversion"),
    },
    {
      ...money(
        'export const c = (a: Money): "x" => (Reflect.ownKeys(a).length === 0 ? "x" : "x");',
      ),
      errors: errors("conversion"),
    },
    ...["toString", "valueOf", "toLocaleString"].map((member) => ({
      ...money(
        `export const c = (a: Money): "x" => (a.${member}() ? "x" : "x");`,
      ),
      errors: errors("member"),
    })),
    {
      code: 'import { moneyKey, type Money } from "../../shared/lib/money";\nexport const c = (a: Money): "x" => (a[moneyKey] === "1" ? "x" : "x");',
      filename: ts,
      options: moneyOptions,
      errors: errors("extraction", "extraction"),
    },
    {
      ...money(
        'export const c = (a: Money): "x" => {\n  const { ...rest } = a;\n  return rest === a ? "x" : "x";\n};',
      ),
      errors: errors("extraction", "operator"),
    },
  ],
});

typed.run("local/no-raw-types", local.rules["no-raw-types"], {
  valid: [
    {
      code: 'export type Side = "buy" | "sell";\nexport const flip = (side: Side): Side => (side === "buy" ? "sell" : "buy");',
      filename: ts,
    },
    {
      code: 'import type { Ticker } from "../../shared/lib/brand";\nexport type Rows = readonly Ticker[];\nexport const first = (rows: Rows): Ticker | undefined => rows[0];',
      filename: ts,
    },
    {
      code: 'export const labels = ["a", "b"] as const;\nexport const count = 3;',
      filename: ts,
    },
    {
      code: 'import type { ReactElement } from "react";\nexport const View = (): ReactElement => <i />;',
      filename: tsx,
    },
    {
      code: 'type Item = { readonly kind: "a" | "b" };\nexport const only = (items: readonly Item[]): readonly Item[] => items.filter((item) => item.kind === "a");',
      filename: ts,
    },
  ],
  invalid: [
    ...["string", "number", "bigint", "boolean", "unknown", "object"].map(
      (keyword) => ({
        code: `export type Box = { readonly value: ${keyword} };`,
        filename: ts,
        errors: errors("resolved", "written"),
      }),
    ),
    {
      code: "export type Box = { readonly value: true };",
      filename: ts,
      errors: errors("resolved", "written"),
    },
    {
      code: "export type Box = { readonly value: String };",
      filename: ts,
      errors: errors("resolved", "written"),
    },
    {
      code: "export type Names = Array<string>;",
      filename: ts,
      errors: errors("resolved", "written"),
    },
    {
      code: 'export type Value = "a" | number;',
      filename: ts,
      errors: errors("written"),
    },
    {
      code: 'import type { Plain } from "../../shared/lib/brand";\nexport type Value = Plain;',
      filename: ts,
      errors: errors("resolved"),
    },
    {
      code: 'import type { plainLabel } from "../../shared/lib/brand";\nexport type Value = ReturnType<typeof plainLabel>;',
      filename: ts,
      errors: errors("resolved", "resolved"),
    },
    {
      code: "export type Value = `${number}`;",
      filename: ts,
      errors: errors("resolved", "written"),
    },
    {
      code: 'import type { PlainRecord } from "../../shared/lib/brand";\nexport type Key = keyof PlainRecord;',
      filename: ts,
      errors: errors("resolved", "resolved"),
    },
    {
      code: 'import { plainLabel } from "../../shared/lib/brand";\nexport const label = plainLabel;',
      filename: ts,
      errors: errors("exported"),
    },
    {
      code: 'import type { ReactElement } from "react";\nimport type { PlainProps } from "../../shared/lib/brand";\nconst Title = (props: PlainProps): ReactElement => <i key={props.title} />;\nexport const View = (): ReactElement => <Title title={"x" as never} />;',
      filename: tsx,
      errors: errors("props", "resolved"),
    },
  ],
});

const messagesFile = at("src/features/home/messages.ts");
const catalogSource =
  'import type { UiText } from "../../shared/lib/brand";\nconst catalog = { "a.title": { ko: "제목" as UiText } } as const;\n';

typed.run("local/no-brand-creation", local.rules["no-brand-creation"], {
  valid: [
    {
      code: 'import { z } from "zod";\nexport const s = z.string().brand<"Ticker">();',
      filename: lib,
      options: [{ mode: "brand-file" }],
    },
    {
      code: `${catalogSource}export const t = (key: "a.title", locale: "ko"): UiText =>\n  catalog[key][locale];`,
      filename: messagesFile,
      options: [{ mode: "messages" }],
    },
    {
      code: 'import type { UiText } from "../../shared/lib/brand";\nexport const catalog = { a: { ko: ("가" + `나`) as UiText } } as const;\nexport type Key = keyof typeof catalog;',
      filename: messagesFile,
      options: [{ mode: "messages" }],
    },
    {
      code: 'export const sides = ["buy"] as const;',
      filename: ts,
      options: [{ mode: "forbid" }],
    },
    {
      code: 'type Item = { readonly brand: "a" };\nexport const b = (item: Item): "a" => item.brand;',
      filename: ts,
      options: [{ mode: "forbid" }],
    },
  ],
  invalid: [
    {
      code: 'import { z } from "zod";\nexport const s = z.string().brand<"Ticker">();',
      filename: ts,
      options: [{ mode: "forbid" }],
      errors: errors("forbidden"),
    },
    {
      code: 'import { z } from "zod";\nconst method = "brand";\nexport const s = z.string()[method]<"Ticker">();',
      filename: ts,
      options: [{ mode: "forbid" }],
      errors: errors("forbidden"),
    },
    {
      code: 'import { z } from "zod";\nexport const b = z.string().brand;',
      filename: ts,
      options: [{ mode: "forbid" }],
      errors: errors("forbidden"),
    },
    {
      code: 'import type { Ticker } from "../../shared/lib/brand";\nexport const t = (x: "1"): Ticker => x as Ticker;',
      filename: ts,
      options: [{ mode: "forbid" }],
      errors: errors("forbidden"),
    },
    {
      code: 'import { z } from "zod";\nimport type { Ticker } from "../../shared/lib/brand";\nexport const s = z.custom<Ticker>();',
      filename: ts,
      options: [{ mode: "forbid" }],
      errors: errors("forbidden"),
    },
    {
      code: 'import type { z } from "zod";\nexport type F = "a" & z.core.$brand<"F">;',
      filename: ts,
      options: [{ mode: "forbid" }],
      errors: errors("forbidden"),
    },
    {
      code: 'import { z } from "zod";\nexport const s = z.string().brand<"UiText">();',
      filename: lib,
      options: [{ mode: "brand-file" }],
      errors: errors("uiText"),
    },
    {
      code: 'export const s = (x: "a"): "b" => x as "b";',
      filename: lib,
      options: [{ mode: "brand-file" }],
      errors: errors("assertion"),
    },
    {
      code: 'import { z } from "zod";\nexport const s = z.string().brand();',
      filename: lib,
      options: [{ mode: "brand-file" }],
      errors: errors("unnamed"),
    },
    {
      code: 'import type { UiText } from "../../shared/lib/brand";\nexport const catalog = { a: { ko: "가" as UiText } } as const;\nexport const s = 1;',
      filename: messagesFile,
      options: [{ mode: "messages" }],
      errors: errors("messagesShape"),
    },
    {
      code: `${catalogSource}const word = "단어";\nexport const t = (key: "a.title"): UiText => catalog[key].ko;\nexport const u = (): UiText => word as UiText;`,
      filename: messagesFile,
      options: [{ mode: "messages" }],
      errors: errors("messagesShape", "messagesShape", "messagesOnly"),
    },
    {
      code: `import type { EntityId } from "../../shared/lib/brand";\n${catalogSource}export const fromId = (id: EntityId): UiText => catalog["a.title"].ko;`,
      filename: messagesFile,
      options: [{ mode: "messages" }],
      errors: errors("messagesShape"),
    },
    {
      code: `${catalogSource}export const t = (key: "a.title"): UiText => catalog[key].ko as UiText;`,
      filename: messagesFile,
      options: [{ mode: "messages" }],
      errors: errors("messagesShape", "messagesOnly"),
    },
    {
      code: 'import { z } from "zod";\nexport const s = z.string().brand<"UiText">();',
      filename: messagesFile,
      options: [{ mode: "messages" }],
      errors: errors("messagesShape", "messagesShape", "messagesOnly"),
    },
  ],
});

const screenFile = at("src/features/text/clean.tsx");
const screen = (code, ...ids) => ({
  code,
  filename: screenFile,
  options: [{ screen: "on" }],
  ...(ids.length === 0 ? {} : { errors: errors(...ids) }),
});

typed.run("local/no-ui-text", local.rules["no-ui-text"], {
  valid: [
    screen('export const route = "home/list";'),
    screen("export const e = <b>\n  <i />\n</b>;"),
    {
      code: "export const e = <b>Label</b>;",
      filename: screenFile,
      options: [{ screen: "off" }],
    },
    screen(
      'import type { UiText } from "../../shared/lib/brand";\nexport const e = (label: UiText, rows: readonly UiText[]) => (\n  <b key="row">\n    {label}\n    {rows}\n  </b>\n);',
    ),
  ],
  invalid: [
    {
      code: 'export const t = "현황";',
      filename: ts,
      options: [{ screen: "off" }],
      errors: errors("hangul"),
    },
    {
      code: "export const t = `현황`;",
      filename: ts,
      options: [{ screen: "off" }],
      errors: errors("hangul"),
    },
    {
      code: 'export type T = "매수" | "매도";',
      filename: ts,
      options: [{ screen: "off" }],
      errors: errors("hangul", "hangul"),
    },
    {
      code: "export const e = <b>현황</b>;",
      filename: screenFile,
      options: [{ screen: "off" }],
      errors: errors("hangul"),
    },
    screen("export const e = <b>Label</b>;", "jsxText"),
    screen('export const e = <b>{"Label"}</b>;', "jsxText"),
    screen("export const e = <>{`Label`}</>;", "jsxText"),
    screen(
      'export const e = (on: "y" | "n") => <b>{on === "y" ? "Yes" : "No"}</b>;',
      "jsxText",
    ),
    screen('export const e = (word: "a") => <b>{"OK " + word}</b>;', "jsxText"),
    screen('export const e = <b>{["Buy now"]}</b>;', "jsxText"),
    screen("export const e = <b>{String.fromCharCode(79)}</b>;", "jsxText"),
  ],
});

typed.run("local/no-promise-catch", local.rules["no-promise-catch"], {
  valid: [
    {
      code: 'export const n = (p: Promise<"a">): Promise<"b"> => p.then((): "b" => "b");',
      filename: ts,
    },
    {
      code: 'type Box = { readonly catch: "c" };\nexport const c = (box: Box): "c" => box.catch;',
      filename: ts,
    },
  ],
  invalid: [
    {
      code: 'export const r = (p: Promise<"a">) => p.catch(() => "b");',
      filename: ts,
      errors: errors("catch"),
    },
    {
      code: 'const name = "catch";\nexport const r = (p: Promise<"a">) => p[name](() => "b");',
      filename: ts,
      errors: errors("catch"),
    },
    {
      code: 'export const r = (p: Promise<"a">) => {\n  const { catch: c } = p;\n  return c;\n};',
      filename: ts,
      errors: errors("catch"),
    },
    {
      code: 'export const r = (p: Promise<"a">) => p.then(() => "b", () => "c");',
      filename: ts,
      errors: errors("then"),
    },
    {
      code: 'export const r = (p: Promise<"a">) => Promise.allSettled([p]);',
      filename: ts,
      errors: errors("allSettled"),
    },
  ],
});

typed.run("local/no-computed-member", local.rules["no-computed-member"], {
  valid: [
    {
      code: 'const labels = { buy: "b", sell: "s" } as const;\nexport const l = (side: "buy" | "sell"): "b" | "s" => labels[side];',
      filename: ts,
    },
    {
      code: 'export const f = (items: readonly "a"[]): "a" | undefined => items[0];',
      filename: ts,
    },
    {
      code: 'export const f = (items: readonly "a"[]): "a" | undefined => items.at(-1);',
      filename: ts,
    },
    {
      code: 'export const f = (box: { readonly a: "a" }): "a" => box["a"];',
      filename: ts,
    },
  ],
  invalid: [
    {
      code: 'export const f = (items: readonly "a"[], i: 0 | 1): "a" | undefined => items[i];',
      filename: ts,
      errors: errors("index"),
    },
    {
      code: 'const key = "size";\nexport const f = (items: ReadonlySet<"a">): "x" => (items[key] === 0 ? "x" : "x");',
      filename: ts,
      errors: errors("target"),
    },
    {
      code: 'import * as brand from "../../shared/lib/brand";\nconst key = "toTicker";\nexport const f = brand[key];',
      filename: ts,
      errors: errors("target"),
    },
    {
      code: 'const k: unique symbol = Symbol("k");\ntype Box = { readonly [k]: "v" };\nexport const f = (box: Box): "v" => box[k];',
      filename: ts,
      errors: errors("key"),
    },
    {
      code: 'type Box = { readonly a: "a" };\nexport const f = <K extends keyof Box>(box: Box, key: K): Box[K] => box[key];',
      filename: ts,
      errors: errors("key"),
    },
    {
      code: 'type Box = { readonly a: "a" };\nexport const f = <K extends keyof Box>(box: Box, key: K): Box[K] => {\n  const { [key]: v } = box;\n  return v;\n};',
      filename: ts,
      errors: errors("key"),
    },
  ],
});

const fileOptions = (checkExtension) => [
  {
    checkExtension,
    placeholders: [".gitkeep"],
    folders: [
      {
        folder: "src/shared/platform",
        files: ["src/shared/platform/index.ts"],
      },
    ],
  },
];

plain.run("local/forbidden-file", local.rules["forbidden-file"], {
  valid: [
    {
      code: "export const a = 1;",
      filename: at("src/shared/platform/index.ts"),
      options: fileOptions(true),
    },
    {
      code: "",
      filename: at("src/features/.gitkeep"),
      options: fileOptions(true),
    },
    {
      code: "export const a = 1;",
      filename: at("src/features/x/upper.TSX"),
      options: fileOptions(false),
    },
    {
      code: "export const a = 1;",
      filename: at("src/features/x/view.tsx"),
      options: fileOptions(true),
    },
  ],
  invalid: [
    {
      code: "export const a = 1;",
      filename: at("src/shared/platform/moved.ts"),
      options: fileOptions(false),
      errors: errors("unlisted"),
    },
    ...["x.TSX", "x.JS", "x.Ts", "x.css", "x.d.ts", "README"].map((name) => ({
      code: "",
      filename: at(`src/features/files/${name}`),
      options: fileOptions(true),
      errors: errors("extension"),
    })),
    {
      code: "x",
      filename: at("src/features/.gitkeep"),
      options: fileOptions(true),
      errors: errors("extension"),
    },
    {
      code: "",
      filename: at("src/shared/platform/notes.md"),
      options: fileOptions(true),
      errors: errors("extension", "unlisted"),
    },
  ],
});

typed.run("local/no-raw-types on library types", local.rules["no-raw-types"], {
  valid: [
    {
      code: 'import type { ChangeEvent, ReactElement, RefObject } from "react";\nexport type Slot = { readonly ref: RefObject<HTMLElement | null>; readonly view: ReactElement };\nexport const handle = (event: Readonly<ChangeEvent<HTMLInputElement>>): void => {\n  event.preventDefault();\n};',
      filename: tsx,
    },
    {
      code: "export const orElse = <T>(value: T | undefined, fallback: NonNullable<T>): NonNullable<T> =>\n  value ?? fallback;",
      filename: ts,
    },
  ],
  invalid: [
    {
      code: 'export type F = Readonly<Pick<HTMLInputElement, "value" | "valueAsNumber" | "checked">>;',
      filename: ts,
      errors: errors("resolved", "resolved"),
    },
    {
      code: 'export type F = Pick<Error, "message">;',
      filename: ts,
      errors: errors("resolved"),
    },
    {
      code: 'export type F = Omit<Error, "stack">;',
      filename: ts,
      errors: errors("resolved"),
    },
    {
      code: "export type F = Partial<Error>;",
      filename: ts,
      errors: errors("resolved"),
    },
    {
      code: "export type F = Required<Error>;",
      filename: ts,
      errors: errors("resolved"),
    },
    {
      code: "export type F = keyof HTMLInputElement;",
      filename: ts,
      errors: errors("resolved"),
    },
    {
      code: 'export type F = HTMLInputElement["value"];',
      filename: ts,
      errors: errors("resolved"),
    },
    {
      code: "export type F = { readonly [K in keyof Error]: Error[K] };",
      filename: ts,
      errors: errors("resolved", "resolved"),
    },
  ],
});
