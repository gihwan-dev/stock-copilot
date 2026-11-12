import { isDeepStrictEqual } from "node:util";

export const appConfigProblems = (config, expected) => {
  const required = { ...expected.options, ...expected.otherOptions };
  const topKeys = Object.keys(config).sort();
  const options = config.compilerOptions ?? {};
  const names = [
    ...new Set([...Object.keys(required), ...Object.keys(options)]),
  ].sort();
  return [
    ...(isDeepStrictEqual(topKeys, ["compilerOptions", "include"])
      ? []
      : [
          `top-level keys: expected compilerOptions, include; got ${topKeys.join(", ")}`,
        ]),
    ...(isDeepStrictEqual(config.include, expected.include)
      ? []
      : [
          `include: expected ${JSON.stringify(expected.include)}, got ${JSON.stringify(config.include)}`,
        ]),
    ...names
      .filter((name) => !isDeepStrictEqual(options[name], required[name]))
      .map(
        (name) =>
          `option ${name}: expected ${JSON.stringify(required[name])}, got ${JSON.stringify(options[name])}`,
      ),
  ];
};
