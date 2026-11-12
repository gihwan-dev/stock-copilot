import { isDeepStrictEqual } from "node:util";
import {
  appSource,
  fixtureExpectations,
  fixtureNames,
  linksUnder,
  withFixture,
} from "./tree.mjs";

const failures = [];
const report = (status, text) => {
  process.stdout.write(`${status} ${text}\n`);
  if (status === "FAIL") failures.push(text);
};

for (const name of fixtureNames) {
  const found = withFixture(name, linksUnder);
  report(
    isDeepStrictEqual(found, fixtureExpectations[name]) ? "PASS" : "FAIL",
    `fixture ${name}: expected [${fixtureExpectations[name].join(", ")}], got [${found.join(", ")}]`,
  );
}

const links = linksUnder(appSource);
links.forEach((link) =>
  report(
    "FAIL",
    `apps/desktop/src/${link}: src 안에는 심볼릭 링크(파일·폴더)를 둘 수 없다`,
  ),
);
if (links.length === 0) report("PASS", "apps/desktop/src has no symbolic link");

process.stdout.write(`source-tree: ${failures.length} failure(s)\n`);
process.exitCode = failures.length === 0 ? 0 : 1;
