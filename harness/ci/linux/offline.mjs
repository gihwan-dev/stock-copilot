import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

export const INNER_SCRIPT =
  'ip link set lo up && exec setpriv --reuid="$1" --regid="$2" --init-groups env HOME="$3" PATH="$4" "${@:5}"';

export const offlineCommand = (identity, command) => [
  "sudo",
  [
    "unshare",
    "--net",
    "bash",
    "-c",
    INNER_SCRIPT,
    "offline",
    identity.uid,
    identity.gid,
    identity.home,
    identity.path,
    ...command,
  ],
];

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const command = process.argv.slice(2);
  if (command.length === 0 || process.platform !== "linux") {
    process.stderr.write(
      "사용법(Linux): node harness/ci/linux/offline.mjs <명령> [인자…]\n",
    );
    process.exitCode = 2;
  } else {
    const [program, args] = offlineCommand(
      {
        uid: String(process.getuid()),
        gid: String(process.getgid()),
        home: process.env.HOME ?? "",
        path: process.env.PATH ?? "",
      },
      command,
    );
    const finished = spawnSync(program, args, { stdio: "inherit" });
    process.exitCode = finished.status ?? 1;
  }
}
