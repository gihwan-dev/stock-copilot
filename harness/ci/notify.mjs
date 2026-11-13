import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

export const issueBody = (runUrl, sha) =>
  `실패한 실행: ${runUrl}\n커밋: ${sha}\n`;

export const existingIssue = (listing, title) => {
  try {
    const found = JSON.parse(listing).find((issue) => issue.title === title);
    return found === undefined ? undefined : String(found.number);
  } catch {
    return undefined;
  }
};

const gh = (args) =>
  spawnSync("gh", args, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "inherit"],
  });

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const repository = process.env.GITHUB_REPOSITORY ?? "";
  const title = process.env.TITLE ?? "";
  const body = issueBody(process.env.RUN_URL ?? "", process.env.SHA ?? "");
  const listing = gh([
    "issue",
    "list",
    "--repo",
    repository,
    "--state",
    "open",
    "--limit",
    "100",
    "--json",
    "number,title",
  ]);
  const number =
    listing.status === 0 ? existingIssue(listing.stdout, title) : undefined;
  const finished =
    listing.status !== 0
      ? listing
      : number === undefined
        ? gh([
            "issue",
            "create",
            "--repo",
            repository,
            "--title",
            title,
            "--body",
            body,
          ])
        : gh([
            "issue",
            "comment",
            number,
            "--repo",
            repository,
            "--body",
            body,
          ]);
  process.stdout.write(finished.stdout ?? "");
  process.exitCode = finished.status ?? 1;
}
