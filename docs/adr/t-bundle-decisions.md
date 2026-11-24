# 토스 묶음 구현 결정 기록 (ADR 0173 경량 절차)

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| T-19 | `broker-toss` 안에 외부 패키지 추가 없이 순수 Rust FIPS 180-4 SHA-256을 두고 `docs/toss-openapi-1.2.28.json`의 원시 해시(`broker_toss_schema::SPEC_SHA256`)와 정규화 JSON 해시를 테스트로 고정 검증한다 | `broker-toss/Cargo.toml`에 `sha2` 의존성 추가 및 `Cargo.lock` 갱신 |
| T-19 | 공개 스펙 조회 전용 `PublicDocsRoute`와 `TossHttpClient::get_public_doc`(비인증 GET, `Last-Modified` 헤더 수집)을 두고, `TossSpecSource`는 `ReadEndpoint`와 분리된 닫힌 `SpecFetchError`를 반환한다 | `ReadRoute`·`ReadEndpoint` 확장(`store-sqlite` 태그 및 OpenAPI 경로 검사 계약까지 연쇄 변경) |
| T-19 | `CheckTossSpec::check`는 드리프트 감지 시 `SpecDriftStatus::DriftDetected { reasons, review: SchemaReviewState::PendingTypeDiffReview }`를 고급 상태 저장소(`TossSpecStatusRepository`)에 기록하고 상태가 바뀔 때만 `DomainEvent::SelfCheckChanged`를 발행하며 주 화면 배너(`SelfCheckAction`)를 만들지 않는다 | `SelfCheckInput`에 스펙 상태를 합쳐 주 화면 경고 경로와 섞음 |
