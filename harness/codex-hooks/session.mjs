import { realpathSync } from "node:fs";
import { dirname } from "node:path";
import { git, loadProject } from "../../.claude/hooks/lib/project.mjs";
export const sessionProject = (cwd = process.cwd()) => {
  const directory = realpathSync(cwd);
  const common = git(directory, [
    "rev-parse",
    "--path-format=absolute",
    "--git-common-dir",
  ])?.trim();
  const gitdir = git(directory, ["rev-parse", "--absolute-git-dir"])?.trim();
  if (!common || !gitdir)
    throw new Error("세션의 git 관리 폴더를 확인하지 못했다");
  const project = loadProject(dirname(realpathSync(common)));
  const root = git(directory, ["rev-parse", "--show-toplevel"])?.trim();
  if (!project.worktrees.includes(root))
    throw new Error("등록된 작업 사본이 아니다");
  return { project, root, cwd: directory, common, gitdir };
};
