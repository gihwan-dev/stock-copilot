export const graphProblems = (graph) => {
  const required = [
    "rusqlite v0.42.0",
    "libsqlite3-sys v0.40.0",
    "sqlcipher-src v420.0.0",
  ];
  const missing = required.filter((name) => !graph.includes(name));
  return [
    ...missing,
    ...(/^openssl(?:-sys|-src)? v/mu.test(graph) ? ["OpenSSL"] : []),
  ];
};

export const linkProblems = (linked) =>
  /lib(?:crypto|ssl)/iu.test(linked) ? ["OpenSSL"] : [];
