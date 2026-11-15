import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { after, test } from "node:test";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const scratch = mkdtempSync(join(tmpdir(), "a02-secret-traits-"));
writeFileSync(
  join(scratch, "Cargo.toml"),
  `[package]\nname = "a02-secret-probe"\nversion = "0.0.0"\nedition = "2024"\n[workspace]\n[lib]\npath = "probe.rs"\n[dependencies]\napplication = { path = ${JSON.stringify(join(root, "crates/application"))} }\nserde = "=1.0.229"\n[dev-dependencies]\nproptest = { version = "=1.12.0", default-features = false, features = ["std"] }\n[[test]]\nname = "debug_properties"\npath = "debug_properties.rs"\n`,
);
copyFileSync(join(root, "Cargo.lock"), join(scratch, "Cargo.lock"));
copyFileSync(
  join(root, "rust-toolchain.toml"),
  join(scratch, "rust-toolchain.toml"),
);
const debugProperties = String.raw`#[cfg(test)]
mod tests {
    use application::SecretValue;
    use proptest::{prelude::any, prop_assert, prop_assert_eq, proptest};

    #[derive(Debug)]
    struct Envelope {
        secret: SecretValue,
    }

    proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(256))]
        #[test]
        fn arbitrary_bytes_and_lengths_have_identical_debug(
            bytes in proptest::collection::vec(any::<u8>(), 0..4097),
            different in proptest::collection::vec(any::<u8>(), 0..4097),
        ) {
            let secret = SecretValue::new(bytes.clone().into());
            let other = SecretValue::new(different.into());
            prop_assert_eq!(format!("{secret:?}"), "SecretValue(<redacted>)");
            prop_assert_eq!(format!("{secret:#?}"), "SecretValue(<redacted>)");
            prop_assert_eq!(format!("{secret:?}"), format!("{other:?}"));
            prop_assert_eq!(format!("{secret:#?}"), format!("{other:#?}"));
            prop_assert_eq!(secret.as_exposed_secret(), bytes.as_slice());
            let envelope = Envelope { secret };
            prop_assert_eq!(format!("{envelope:?}"), "Envelope { secret: SecretValue(<redacted>) }");
            prop_assert_eq!(format!("{envelope:#?}"), "Envelope {\n    secret: SecretValue(<redacted>),\n}");
            prop_assert_eq!(envelope.secret.as_exposed_secret(), bytes.as_slice());
        }

        #[test]
        fn marked_payload_is_absent_from_direct_and_parent_debug(
            payload in proptest::collection::vec(any::<u8>(), 0..4097),
        ) {
            let mut bytes = b"\0A02_FAKE_SECRET_".to_vec();
            bytes.extend(payload);
            let secret = SecretValue::new(bytes.clone().into());
            let normal = format!("{secret:?}");
            let pretty = format!("{secret:#?}");
            let envelope = Envelope { secret };
            let byte_representation = format!("{bytes:?}");
            for rendered in [normal, pretty, format!("{envelope:?}"), format!("{envelope:#?}")] {
                prop_assert!(!rendered.as_bytes().windows(bytes.len()).any(|window| window == bytes.as_slice()));
                prop_assert!(!rendered.contains("A02_FAKE_SECRET_"));
                prop_assert!(!rendered.contains(&byte_representation));
            }
            prop_assert_eq!(envelope.secret.as_exposed_secret(), bytes.as_slice());
        }
    }
}
`;
writeFileSync(join(scratch, "debug_properties.rs"), debugProperties);
after(() => rmSync(scratch, { recursive: true, force: true }));
function compile(body, locked = true) {
  writeFileSync(join(scratch, "probe.rs"), body);
  return spawnSync(
    "cargo",
    ["check", "--offline", ...(locked ? ["--locked"] : [])],
    {
      encoding: "utf8",
      cwd: scratch,
      env: {
        ...process.env,
        CARGO_BUILD_JOBS: "1",
        CARGO_TARGET_DIR: join(scratch, "target"),
      },
    },
  );
}

test("외부 소비자는 명시적 메서드 한 곳으로만 비밀을 읽는다", () => {
  const result = compile(
    "pub fn probe(value: application::SecretValue) { let _: &[u8] = value.as_exposed_secret(); }",
    false,
  );
  assert.equal(result.status, 0, result.stderr);
});
for (const bound of ["std::fmt::Display", "Clone", "serde::Serialize"]) {
  test(`SecretValue는 ${bound}를 구현하지 않는다`, () => {
    const control = compile(
      `fn needs<T: ${bound}>() {} pub fn probe() { needs::<String>(); }`,
    );
    assert.equal(control.status, 0, control.stderr);
    const result = compile(
      `fn needs<T: ${bound}>() {} pub fn probe() { needs::<application::SecretValue>(); }`,
    );
    assert.equal(result.status, 101, result.stderr);
    assert.match(result.stderr, /E0277/);
    assert.match(result.stderr, /SecretValue/);
  });
}
test("비밀 래퍼의 내부 필드를 꺼낼 수 없다", () => {
  const result = compile(
    "pub fn probe(value: application::SecretValue) { let _ = value.0; }",
  );
  assert.equal(result.status, 101, result.stderr);
  assert.match(result.stderr, /E0616/);
});
test("의존성은 정확한 버전이며 secrecy serde 기능을 켜지 않는다", () => {
  const manifest = readFileSync(
    join(root, "crates/application/Cargo.toml"),
    "utf8",
  );
  assert.match(manifest, /^secrecy = "=0\.10\.3"$/m);
  assert.doesNotMatch(manifest, /secrecy[^\n]*serde/);
  const tree = spawnSync(
    "cargo",
    ["tree", "--locked", "-p", "secrecy", "-e", "features"],
    { cwd: root, encoding: "utf8" },
  );
  assert.equal(tree.status, 0, tree.stderr);
  assert.doesNotMatch(tree.stdout, /serde|zeroize_derive/);
});

test("고정 Debug는 임의 바이트·길이와 derive(Debug) 부모에서도 유지된다", () => {
  const control = compile("pub fn property_control() {} ");
  assert.equal(control.status, 0, control.stderr);
  const result = spawnSync(
    "cargo",
    ["test", "--offline", "--locked", "--test", "debug_properties"],
    {
      encoding: "utf8",
      cwd: scratch,
      env: {
        ...process.env,
        CARGO_BUILD_JOBS: "1",
        CARGO_TARGET_DIR: join(scratch, "target"),
      },
    },
  );
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.match(result.stdout, /2 passed; 0 failed/);
});
