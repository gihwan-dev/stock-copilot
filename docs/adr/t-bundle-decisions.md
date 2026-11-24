# 토스 묶음 구현 결정 기록 (ADR 0173 경량 절차)

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| T-19 | `broker-toss` 안에 외부 패키지 추가 없이 순수 Rust FIPS 180-4 SHA-256을 두고 `docs/toss-openapi-1.2.28.json`의 원시 해시(`broker_toss_schema::SPEC_SHA256`)와 정규화 JSON 해시를 테스트로 고정 검증한다 | `broker-toss/Cargo.toml`에 `sha2` 의존성 추가 및 `Cargo.lock` 갱신 |
| T-19 | 공개 스펙 조회 전용 `PublicDocsRoute`와 `TossHttpClient::get_public_doc`(비인증 GET, `Last-Modified` 헤더 수집)을 두고, `TossSpecSource`는 `ReadEndpoint`와 분리된 닫힌 `SpecFetchError`를 반환한다 | `ReadRoute`·`ReadEndpoint` 확장(`store-sqlite` 태그 및 OpenAPI 경로 검사 계약까지 연쇄 변경) |
| T-19 | `CheckTossSpec::check`는 드리프트 감지 시 `SpecDriftStatus::DriftDetected { reasons, review: SchemaReviewState::PendingTypeDiffReview }`를 고급 상태 저장소(`TossSpecStatusRepository`)에 기록하고 상태가 바뀔 때만 `DomainEvent::SelfCheckChanged`를 발행하며 주 화면 배너(`SelfCheckAction`)를 만들지 않는다 | `SelfCheckInput`에 스펙 상태를 합쳐 주 화면 경고 경로와 섞음 |
| T-17 | 새 외부 웹소켓 패키지(`tokio-tungstenite` 등)를 추가하지 않고 `reqwest 0.13.5`의 HTTP/1.1 `Response::upgrade()`와 순수 Rust RFC 6455 프레임 코덱(`ws_frame.rs`)으로 구현해 `https_only(true)`·`no_proxy()`·`LoopbackResolver`·`arch-check` `http-socket` 제약을 그대로 유지한다 | `tokio-tungstenite`·`tungstenite`를 새로 추가하고 `deny.toml`·`Cargo.lock`·`arch-check` 예외를 확장 |
| T-17 | 웹소켓 전용 `TossWsEndpoint`(`https://openapi-ws.tossinvest.com`, `local(port)`)와 `TossWsConnector`(전체 타임아웃 없는 `configured_ws_client`)를 `endpoint.rs`·`http.rs`의 동일 닫힌 가드(`arch_check/source/endpoint.rs`) 아래 둔다 | `TossEndpoint`에 URL 치환 메서드를 추가하거나 전체 30초 타임아웃 `TossHttpClient`를 공유 |
| T-17 | `TossWsClient::reconcile_orders_with_token`은 전달받은 토큰으로 `TossHttpClient::get`을 직접 호출하고, `Arc<TossWsClient>`에 `TokenRenewal`을 구현해 `TokenManager`의 만료·재발급 경로(발급→닫기→재연결→재선언→주문 재조회)에서 교착 없이 동작하게 한다 | `TokenRenewal` 내부에서 `TokenManager::get`을 재귀 호출하거나 별도 브리지 래퍼 타입 추가 |
| A-26 | `domain`에 소유형 날짜별 입력 `ValuationDayInputs`(행 집합·이력·`DayCashLedger` 둘·관측)와 `ValuationGapPlan::excluding`을 두고, `application::BackfillValuationDays`가 계획 → 성공일 조회(`ValuationDaySuccessSource`) → 제외 → 날짜별 입력 수집(`ValuationDayInputSource`) → 재계산 → 날짜별 기록 순으로 돈다 | 입력 공급 포트가 빌린 `ValuationRecomputeInput`을 직접 돌려주게 함(수명 때문에 비동기 포트에 맞지 않음) |
| A-26 | 입력 포트의 `ValuationInputError::Missing(SnapshotError)`는 그 날짜만 실패로 기록하고, `Port(PortError)`·저장소 오류는 그 자리에서 실행을 멈춘다(앞서 기록한 날은 남고 다음 실행에서 이어 감). 성공일만 제외하므로 실패일은 다음 실행에서 다시 시도한다 | 일시 장애도 그 날짜의 실패로 기록 |
