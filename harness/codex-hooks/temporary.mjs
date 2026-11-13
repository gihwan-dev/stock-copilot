import { createHash } from "node:crypto";
import { lstatSync, mkdirSync, realpathSync, rmSync } from "node:fs";
import { join } from "node:path";

export const sessionTemporary = (worktree) =>
  join(
    process.platform === "darwin" ? "/private/tmp" : "/tmp",
    "stock-check-" +
      process.getuid() +
      "-" +
      createHash("sha256")
        .update(realpathSync(worktree))
        .digest("hex")
        .slice(0, 16),
  );
export const prepareDirectory = (path) => {
  mkdirSync(path, { recursive: true, mode: 0o700 });
  const info = lstatSync(path);
  if (
    !info.isDirectory() ||
    info.isSymbolicLink() ||
    info.uid !== process.getuid() ||
    (info.mode & 0o077) !== 0
  )
    throw new Error("검사 임시 폴더의 소유자나 권한이 잘못됐다");
  return path;
};
export const prepareTemporary = (worktree) =>
  prepareDirectory(sessionTemporary(worktree));
export const removeTemporary = (worktree) =>
  rmSync(sessionTemporary(worktree), { recursive: true, force: true });
