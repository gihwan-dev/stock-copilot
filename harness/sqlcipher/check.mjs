import { graphProblems, linkProblems } from "./rules.mjs";
import { mkdirSync } from "node:fs";
import { execFileSync } from "node:child_process";

const graph = execFileSync(
  "cargo",
  [
    "tree",
    "--locked",
    "-p",
    "store-sqlite",
    "--edges",
    "normal,build",
    "--prefix",
    "none",
  ],
  { encoding: "utf8" },
);
const problems = graphProblems(graph);
if (problems.length !== 0)
  throw new Error(`SQLCipher 의존성 검사: ${problems.join(", ")}`);
process.stdout.write("sqlcipher: 고정된 번들 의존성, OpenSSL 없음\n");

if (process.platform === "darwin") {
  mkdirSync("target/S-01", { recursive: true });
  const artifacts = execFileSync(
    "cargo",
    ["build", "--locked", "-p", "store-sqlite", "--message-format=json"],
    { encoding: "utf8" },
  )
    .trim()
    .split("\n")
    .map((line) => JSON.parse(line));
  const library = (name) =>
    artifacts
      .find(
        (item) =>
          item.reason === "compiler-artifact" && item.target.name === name,
      )
      .filenames.find((file) => file.endsWith(".rlib"));
  execFileSync("rustc", [
    "--test",
    "--edition=2024",
    "harness/sqlcipher/debug.rs",
    "--extern",
    `application=${library("application")}`,
    "--extern",
    `store_sqlite=${library("store_sqlite")}`,
    "-L",
    "dependency=target/debug/deps",
    "-o",
    "target/S-01/debug-probe",
  ]);
  const linked = execFileSync("otool", ["-L", "target/S-01/debug-probe"], {
    encoding: "utf8",
  });
  if (linkProblems(linked).length !== 0)
    throw new Error("SQLCipher 산출물에 OpenSSL이 링크됐다");
  execFileSync("target/S-01/debug-probe");
  process.stdout.write(
    "sqlcipher: 산출물 OpenSSL 미링크, 비밀 Debug·오류 검사 통과\n",
  );
}
