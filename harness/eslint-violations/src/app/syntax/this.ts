export const holder = {
  value: "a",
  read(): unknown {
    return this.value;
  },
} as const;
