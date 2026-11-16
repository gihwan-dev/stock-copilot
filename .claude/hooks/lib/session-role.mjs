import { dirname } from "node:path";
import { commonDirectory } from "./git-facts.mjs";
import { HOOKS_PROJECT_ROOT } from "./project.mjs";

export const claudeSessionInput = (input, root = HOOKS_PROJECT_ROOT) => {
  const common = commonDirectory(root);
  if (common === undefined) {
    return { ...input, agent_type: "unknown", agent_id: "unverified-session" };
  }
  return root === dirname(common)
    ? input
    : { ...input, agent_type: "adversarial-reviewer", agent_id: "t3-reviewer" };
};
