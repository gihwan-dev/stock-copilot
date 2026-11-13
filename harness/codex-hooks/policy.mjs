const failure = (kind, path) => {
  throw new Error("Codex 정책 대응을 정하지 않았다: " + kind + " " + path);
};
const row = (kind, path, target, mode, scope, reason) => ({
  kind,
  path,
  target,
  mode,
  scope,
  reason,
});
export const policyCoverage = (policy) => {
  const filesystem = policy.base.filesystem;
  if (
    Object.keys(filesystem).some(
      (key) =>
        !["allowWrite", "denyWrite", "denyRead", "allowRead"].includes(key),
    )
  )
    failure("filesystem", "새 항목");
  const rows = Object.entries(filesystem).flatMap(([kind, paths]) =>
    paths.map((path) => {
      if (kind === "allowWrite") {
        if (path === "../stock-copilot-wt")
          return row(
            kind,
            path,
            ":workspace_roots",
            "write",
            "narrower",
            "현재 세션 작업 사본만 쓰고 등록된 다른 작업 사본은 읽기 전용이다",
          );
        if (/^~\/Library\/pnpm\/_tmp_\*(?:\/\*)?$/u.test(path))
          return row(
            kind,
            path,
            "pnpm_config_store_dir",
            "read",
            "narrower",
            "부모 폴더의 임시 쓰기는 열지 않고 고정한 저장소 안에서 설치한다",
          );
        if (
          /^\/private\/tmp\/pnpm-(?:store-operation-locks|dir-locks|modules-swaps)-[0-9]+$/u.test(
            path,
          )
        )
          return row(
            kind,
            path,
            ":slash_tmp",
            "write",
            "inherited",
            "Codex 기본 임시 폴더 쓰기가 이 경로를 포함한다",
          );
        if (path.startsWith("~/") && !path.includes("*"))
          return row(
            kind,
            path,
            path,
            "write",
            "filesystem",
            "홈 축약 경로 쓰기를 그대로 적용한다",
          );
        return failure(kind, path);
      }
      if (kind === "denyWrite" && path.startsWith("../stock-copilot-wt/"))
        return row(
          kind,
          path,
          "registered-other-worktrees",
          "read",
          "narrower",
          "다른 작업 사본 전체를 읽기 전용으로 출력한다",
        );
      const mode = kind === "denyRead" ? "deny" : "read";
      if (path.startsWith("~/"))
        return row(
          kind,
          path,
          path,
          mode,
          "filesystem",
          "홈 경로와 읽기 차단 glob을 그대로 적용한다",
        );
      if (path.startsWith("./") && kind === "denyWrite")
        return row(
          kind,
          path,
          path.slice(2),
          mode,
          "workspace",
          "세션 작업 사본의 상대 경로를 읽기 전용으로 적용한다",
        );
      return failure(kind, path);
    }),
  );
  for (const path of policy.base.network.allowUnixSockets) {
    if (!/^\/private\/tmp\/claude-[0-9]+$/u.test(path))
      failure("allowUnixSockets", path);
    rows.push(
      row(
        "allowUnixSockets",
        path,
        "setup:temporary",
        "deny",
        "narrower",
        "Claude 전용 소켓은 열지 않는다. Codex 검사의 임시 소켓 범위는 사용자 설정 출력에서 별도로 지정한다",
      ),
    );
  }
  return rows;
};
