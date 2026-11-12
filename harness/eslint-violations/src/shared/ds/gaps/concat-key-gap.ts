const parts = ["default", "View"] as const;
export const view = (): unknown => document[`${parts[0]}${parts[1]}`];
