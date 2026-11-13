import { strict as assert } from "node:assert";
import { test } from "node:test";
import { checkPnpm, pnpmVersion } from "../../toolchain/pnpm.mjs";

test("pnpm check rejects an old shim a wrong version and a missing pin", () => {
  const manifest = { packageManager: "pnpm@12.18.0+sha512.example" };
  assert.equal(pnpmVersion(manifest), "12.18.0");
  assert.equal(
    checkPnpm(manifest, () => ({ status: 0, stdout: "12.18.0\n" })),
    undefined,
  );
  for (const result of [
    { status: 1, stderr: "MODULE_NOT_FOUND" },
    { status: 0, stdout: "9.0.0" },
  ])
    assert.match(
      checkPnpm(manifest, () => result),
      /PATH.*Library\/pnpm/u,
    );
  assert.throws(() => pnpmVersion({}));
});
