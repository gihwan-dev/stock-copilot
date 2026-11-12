export const ROLE = Object.freeze({
  main: "main",
  implementer: "implementer",
  reviewer: "reviewer",
  unknown: "unknown",
});

const SUBAGENT_ROLES = new Map([
  ["implementer", ROLE.implementer],
  ["adversarial-reviewer", ROLE.reviewer],
  ["review-critic", ROLE.reviewer],
]);

const absent = (value) => value === undefined || value === null;

export const roleOf = (input) => {
  if (absent(input.agent_type) && absent(input.agent_id)) {
    return ROLE.main;
  }
  return SUBAGENT_ROLES.get(input.agent_type) ?? ROLE.unknown;
};

export const isSubagent = (role) => role !== ROLE.main;
