import assert from "node:assert/strict";
import test from "node:test";
import { cpSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { mkdtempSync } from "./ds-runtime-environment.mjs";
import { desktopRoot } from "./paths.mjs";

const validExpressions = [
  "<Page title={label}><Text>{label}</Text></Page>",
  "<Page title={label} actions={action}><Section title={label}><Text>{label}</Text></Section></Page>",
  "<Section title={label}><Text>{label}</Text><Text>{label}</Text></Section>",
  "<Grid><GridItem span={3}><Text>{label}</Text></GridItem><GridItem span={4}><Text>{label}</Text></GridItem><GridItem span={6}><Text>{label}</Text></GridItem><GridItem span={8}><Text>{label}</Text></GridItem><GridItem span={9}><Text>{label}</Text></GridItem><GridItem span={12}><Text>{label}</Text></GridItem></Grid>",
  "<Grid><GridItem span={{ narrow: 12 }}><Text>{label}</Text></GridItem><GridItem span={{ medium: 6 }}><Text>{label}</Text></GridItem><GridItem span={{ wide: 4 }}><Text>{label}</Text></GridItem><GridItem span={{ narrow: 12, medium: 6, wide: 4 }}><Text>{label}</Text></GridItem></Grid>",
  '<Stack><Text>{label}</Text></Stack><Stack gap="space-1"><Text>{label}</Text><Text>{label}</Text></Stack><Stack gap="space-12"><Text>{label}</Text></Stack>',
  '<Inline><Text>{label}</Text></Inline><Inline gap="space-4" align="start" justify="between" wrap="wrap"><Text>{label}</Text><Text>{label}</Text></Inline><Inline align="center" justify="center" wrap="nowrap"><Text>{label}</Text></Inline><Inline align="end" justify="end" wrap="wrap"><Text>{label}</Text></Inline>',
  '<Split size="sidebar-width"><Text>{label}</Text><Text>{label}</Text></Split><Split size="sidebar-collapsed"><Text>{label}</Text><Text>{label}</Text></Split><Split size="chat-width"><Text>{label}</Text><Text>{label}</Text></Split><Split size="chat-width-min"><Text>{label}</Text><Text>{label}</Text></Split>',
  '<Toolbar><Button>{label}</Button><Divider direction="vertical" /></Toolbar>',
  '<Divider /><Divider direction="horizontal" /><Divider direction="vertical" />',
];

const invalidCases = [
  ["GridItem span 0", "<GridItem span={0}><Text>{label}</Text></GridItem>"],
  ["GridItem span 1", "<GridItem span={1}><Text>{label}</Text></GridItem>"],
  ["GridItem span 2", "<GridItem span={2}><Text>{label}</Text></GridItem>"],
  ["GridItem span 5", "<GridItem span={5}><Text>{label}</Text></GridItem>"],
  ["GridItem span 7", "<GridItem span={7}><Text>{label}</Text></GridItem>"],
  ["GridItem span 10", "<GridItem span={10}><Text>{label}</Text></GridItem>"],
  ["GridItem span 11", "<GridItem span={11}><Text>{label}</Text></GridItem>"],
  ["GridItem span 13", "<GridItem span={13}><Text>{label}</Text></GridItem>"],
  [
    "GridItem computed span",
    "<GridItem span={3 + 3}><Text>{label}</Text></GridItem>",
  ],
  [
    "GridItem string span",
    '<GridItem span="4"><Text>{label}</Text></GridItem>',
  ],
  [
    "GridItem empty responsive object",
    "<GridItem span={{}}><Text>{label}</Text></GridItem>",
  ],
  [
    "GridItem invalid narrow span",
    "<GridItem span={{ narrow: 5 }}><Text>{label}</Text></GridItem>",
  ],
  [
    "GridItem invalid medium span",
    "<GridItem span={{ medium: 7 }}><Text>{label}</Text></GridItem>",
  ],
  [
    "GridItem invalid wide span",
    "<GridItem span={{ wide: 10 }}><Text>{label}</Text></GridItem>",
  ],
  [
    "GridItem unknown band key",
    "<GridItem span={{ tablet: 6 }}><Text>{label}</Text></GridItem>",
  ],
  ["GridItem missing span", "<GridItem><Text>{label}</Text></GridItem>"],
  [
    "GridItem className",
    '<GridItem span={6} className="col-span-6"><Text>{label}</Text></GridItem>',
  ],
  ["Page raw string title", '<Page title="화면"><Text>{label}</Text></Page>'],
  [
    "Page extra leading prop",
    "<Page title={label} leading={<Button>{label}</Button>}><Text>{label}</Text></Page>",
  ],
  [
    "Page extra status prop",
    "<Page title={label} status={<Text>{label}</Text>}><Text>{label}</Text></Page>",
  ],
  [
    "Page arbitrary element actions",
    "<Page title={label} actions={<Button>{label}</Button>}><Text>{label}</Text></Page>",
  ],
  [
    "Page className",
    '<Page title={label} className="bg-bg"><Text>{label}</Text></Page>',
  ],
  ["Section missing title", "<Section><Text>{label}</Text></Section>"],
  [
    "Section raw string title",
    '<Section title="구역"><Text>{label}</Text></Section>',
  ],
  [
    "Section extra actions prop",
    "<Section title={label} actions={action}><Text>{label}</Text></Section>",
  ],
  [
    "Section style prop",
    "<Section title={label} style={{ gap: 8 }}><Text>{label}</Text></Section>",
  ],
  [
    "Grid extra columns prop",
    "<Grid columns={12}><GridItem span={12}><Text>{label}</Text></GridItem></Grid>",
  ],
  [
    "Grid extra gap prop",
    '<Grid gap="space-4"><GridItem span={12}><Text>{label}</Text></GridItem></Grid>',
  ],
  [
    "Stack invalid gap token",
    '<Stack gap="space-13"><Text>{label}</Text></Stack>',
  ],
  [
    "Stack foreign token gap",
    '<Stack gap="sidebar-width"><Text>{label}</Text></Stack>',
  ],
  [
    "Stack extra align prop",
    '<Stack align="center"><Text>{label}</Text></Stack>',
  ],
  [
    "Inline disallowed align baseline",
    '<Inline align="baseline"><Text>{label}</Text></Inline>',
  ],
  [
    "Inline disallowed align stretch",
    '<Inline align="stretch"><Text>{label}</Text></Inline>',
  ],
  [
    "Inline invalid justify",
    '<Inline justify="around"><Text>{label}</Text></Inline>',
  ],
  [
    "Inline boolean wrap true",
    "<Inline wrap={true}><Text>{label}</Text></Inline>",
  ],
  [
    "Inline boolean wrap false",
    "<Inline wrap={false}><Text>{label}</Text></Inline>",
  ],
  [
    "Inline foreign gap token",
    '<Inline gap="chat-width"><Text>{label}</Text></Inline>',
  ],
  [
    "Split disallowed dialog-width",
    '<Split size="dialog-width"><Text>{label}</Text><Text>{label}</Text></Split>',
  ],
  [
    "Split disallowed prose-max",
    '<Split size="prose-max"><Text>{label}</Text><Text>{label}</Text></Split>',
  ],
  [
    "Split disallowed content-max",
    '<Split size="content-max"><Text>{label}</Text><Text>{label}</Text></Split>',
  ],
  [
    "Split foreign spacing token",
    '<Split size="space-4"><Text>{label}</Text><Text>{label}</Text></Split>',
  ],
  [
    "Split missing size",
    "<Split><Text>{label}</Text><Text>{label}</Text></Split>",
  ],
  [
    "Split extra side prop",
    '<Split size="sidebar-width" side="end"><Text>{label}</Text><Text>{label}</Text></Split>',
  ],
  [
    "Split extra gap prop",
    '<Split size="sidebar-width" gap="space-4"><Text>{label}</Text><Text>{label}</Text></Split>',
  ],
  [
    "Split extra align prop",
    '<Split size="sidebar-width" align="center"><Text>{label}</Text><Text>{label}</Text></Split>',
  ],
  [
    "Split single child",
    '<Split size="sidebar-width"><Text>{label}</Text></Split>',
  ],
  [
    "Split three children",
    '<Split size="sidebar-width"><Text>{label}</Text><Text>{label}</Text><Text>{label}</Text></Split>',
  ],
  [
    "Toolbar extra label prop",
    "<Toolbar label={label}><Button>{label}</Button></Toolbar>",
  ],
  [
    "Toolbar role prop",
    '<Toolbar role="toolbar"><Button>{label}</Button></Toolbar>',
  ],
  [
    "Toolbar className prop",
    '<Toolbar className="flex"><Button>{label}</Button></Toolbar>',
  ],
  ["Divider invalid direction", '<Divider direction="both" />'],
  ["Divider children forbidden", "<Divider><Text>{label}</Text></Divider>"],
  ["Divider className", '<Divider className="bg-line" />'],
];

test("DS11 layout component types enforce architecture 5.1 contracts in both tsc and tsc6", () => {
  const folder = mkdtempSync("ds11-layout-types-");
  try {
    cpSync(join(desktopRoot, "src"), join(folder, "src"), { recursive: true });
    const project = join(folder, "tsconfig.json");
    const probe = join(folder, "src/shared/ds/LayoutTypeProbe.tsx");
    writeFileSync(
      project,
      JSON.stringify({
        extends: join(desktopRoot, "tsconfig.json"),
        compilerOptions: {
          rootDir: "src",
          noUnusedLocals: false,
          noUnusedParameters: false,
        },
        include: ["src"],
      }),
    );
    const header = [
      'import type { ReactElement } from "react";',
      'import { Button, Divider, Grid, GridItem, Inline, Page, Section, Split, Stack, Text, Toolbar } from "./index";',
      'import { text } from "../lib";',
      'const label = text("app.title", "ko");',
      "const action = { label, onPress: (): void => undefined };",
    ].join("\n");
    const headerLines = header.split("\n").length;
    const invalidSource = invalidCases
      .map(
        ([, expr], index) =>
          `export const Invalid_${index} = (): ReactElement => ${expr};`,
      )
      .join("\n");
    const compile = (binary) =>
      spawnSync(
        join(desktopRoot, "node_modules/.bin", binary),
        ["-p", project],
        {
          encoding: "utf8",
        },
      );
    for (const binary of ["tsc", "tsc6"]) {
      writeFileSync(
        probe,
        `${header}\nexport const Probe = (): ReactElement => <>${validExpressions.join("")}</>;\n`,
      );
      const accepted = compile(binary);
      assert.equal(
        accepted.status,
        0,
        `${binary}: ${accepted.stdout}${accepted.stderr}`,
      );
      writeFileSync(probe, `${header}\n${invalidSource}\n`);
      const rejected = compile(binary);
      assert.notEqual(rejected.status, 0, binary);
      const errorLines = new Set(
        Array.from(
          rejected.stdout.matchAll(/LayoutTypeProbe\.tsx\((\d+),/gu),
          (match) => Number(match[1]),
        ),
      );
      for (const [index, [name]] of invalidCases.entries()) {
        const expectedLine = headerLines + 1 + index;
        assert.ok(
          errorLines.has(expectedLine),
          `${binary} accepted invalid case (${name}) at line ${expectedLine}\n${rejected.stdout}`,
        );
      }
      writeFileSync(
        probe,
        'import { ScrollArea } from "./index";\nexport const leak = ScrollArea;\n',
      );
      const hidden = compile(binary);
      assert.notEqual(
        hidden.status,
        0,
        `${binary} allowed ScrollArea from index`,
      );
      assert.match(hidden.stdout, /TS2305|TS2724/u, binary);
    }
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});
