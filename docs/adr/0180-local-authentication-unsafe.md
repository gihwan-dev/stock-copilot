# ADR 0180: 본인 확인(LocalAuthentication)의 `unsafe` 예외는 `platform-macos` 한 곳에 둔다 (O-07)

- 상태: 확정 (2026-11-26, fin-native 묶음). 사용자가 `objc2-local-authentication` 추가와 이 예외를 직접 승인했다. architecture 4.5가 열어 둔 "안전한 래퍼냐, 보호된 예외냐"를 정한다.

## 배경

주문 마지막 단계의 본인 확인은 `LAContext.evaluatePolicy(.deviceOwnerAuthentication)`이다(O-07, SEC-8). Rust 바인딩 `objc2-local-authentication =0.3.2`는 `LAContext::new`, `canEvaluatePolicy:error:`, `evaluatePolicy:localizedReason:reply:`를 모두 `unsafe fn`으로 낸다. 같은 기능을 `unsafe` 없이 감싼 crate는 잠금 파일에 없고, 새 래퍼 crate를 들이는 것은 승인 범위 밖이다.

작업 공간은 `unsafe_code = "forbid"`다. `forbid`는 소스의 `#[allow]`·`#[expect]`로 풀 수 없고, `#[allow]`는 `clippy::allow_attributes`(forbid)와 source-check가 막는다.

## 결정

1. `crates/adapters/platform-macos/Cargo.toml`만 `[lints] workspace = true` 대신 작업 공간 린트 표를 그대로 옮기고 `unsafe_code`만 `"deny"`로 둔다.
2. source-check의 새 허용 목록 `[[unsafe-lints]]`(`harness/source-checks/allowlists.toml`)에 이 manifest 하나만 올린다. 목록에 있어도 린트 표가 "작업 공간 표에서 `rust.unsafe_code`만 `forbid → deny`"와 한 글자라도 다르면 `manifest:lints-not-inherited` 위반이다. 작업 공간 표를 바꾸면 이 manifest도 같이 바꿔야 통과한다.
3. `unsafe`는 `src/local_auth.rs`의 `let` 문 세 개(`LAContext::new`, `canEvaluatePolicy_error`, `evaluatePolicy_localizedReason_reply`)에만 있고, 문마다 이유를 단 `#[expect(unsafe_code, reason = "O-07: …")]`를 붙인다. disable-control 허용 목록(`harness/disable-control/allowlist.toml`)에 그 세 문을 문장 그대로 올린다. 다른 곳의 `#[expect(unsafe_code)]`는 disable-check가 `not-disableable`로 거부한다.
4. 모듈 단위 `#[allow(unsafe_code)]`는 쓰지 않는다. `allow`는 금지 속성이고 disable-control은 모듈 위치의 끄기를 막는다. 문 단위 `expect`가 더 좁다.
5. 호출은 이름 붙은 OS 스레드(`device-owner-auth`)에서 하고, 결과는 채널과 waker로 비동기 포트(`DeviceOwnerAuthBackend::evaluate`)에 돌려준다(tasks O-07의 "블로킹 호출은 실행기 밖에서"). 응답 블록은 `Send`인 채널 송신자만 잡고, `LAContext`는 응답이 올 때까지 그 스레드가 붙잡는다. 오류 객체(`*mut NSError`)는 읽지 않는다: 미리 묻기가 실패하면 `Unavailable`, 평가가 실패하면 `Denied`, 응답 없이 끝나면 `Unavailable`이다. 빈 이유 문자열은 시스템 창을 부르지 않고 `Unavailable`이다(LocalAuthentication이 예외를 던지는 입력).
6. 시험은 실제 Touch ID를 부르지 않는다. 응답 대응표, 빈 이유 문자열, 스레드-future 전달만 단위 시험하고, 확인 흐름은 기존 `testkit-ports`의 가짜 `UserPresence`로 본다.

## 결과

- `objc2`·`block2`·`objc2-foundation`·`objc2-local-authentication`의 빌드 의존(`objc2-exception-helper → cc → jobserver → getrandom`의 wasm 대상)과 `bitflags → serde_core`의 `serde_derive` 버전 고정 경로 때문에 `arch-check`가 `platform-macos => syn, proc-macro2`를 잡는다. `harness/arch-check/rules.toml`에 네 crate 각각을 `through`로 한 `syn`·`proc-macro2` 예외를 둔다.
- 다른 crate에는 예외가 없다. `order-execution`은 여전히 `forbid`다(architecture 3.2 R-09 Q4).

## 다른 선택지

- 작업 공간을 `deny`로 낮추기: 모든 crate에서 `#[expect(unsafe_code)]`가 컴파일된다. 하네스의 forbid 고정 시험(`rust_lints_tests`)도 깨진다.
- 새 FFI 전용 crate: 의존 방향·구조 검사·CI 제외 목록 등록이 늘고, architecture 4.5의 "`platform-macos` 하나"와 어긋난다.
- 안전한 래퍼 crate: 승인된 패키지 밖이다.
