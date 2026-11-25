import type { Locale, UiText } from "../../shared/lib";

const catalog = {
  "onboarding.loading": { ko: "토스 연결을 확인하는 중이에요" as UiText },
  "onboarding.loadingBody": { ko: "잠시만 기다려 주세요." as UiText },
  "onboarding.progress": { ko: "시작하기 진행 단계" as UiText },
  "onboarding.keyTitle": { ko: "토스증권 키를 붙여 넣어 주세요" as UiText },
  "onboarding.keyBody": {
    ko: "WTS의 설정 > Open API에서 받은 client_id와 secret을 붙여 넣으세요." as UiText,
  },
  "onboarding.clientIdLabel": { ko: "client_id" as UiText },
  "onboarding.secretLabel": { ko: "secret" as UiText },
  "onboarding.connect": { ko: "연결하기" as UiText },
  "onboarding.later": { ko: "나중에 할게요" as UiText },
  "onboarding.invalidField": {
    ko: "키 형식이 맞지 않아요. 다시 복사해 붙여 넣으세요." as UiText,
  },
  "onboarding.invalidKey": {
    ko: "토스 키가 맞지 않습니다. WTS의 Open API 설정에서 키를 다시 복사해 붙여 넣으세요." as UiText,
  },
  "onboarding.forbidden": { ko: "토스 키의 권한을 확인하세요." as UiText },
  "onboarding.accountNotFound": {
    ko: "토스에서 계좌 상태를 확인하세요." as UiText,
  },
  "onboarding.keyInUse": {
    ko: "같은 토스 키를 쓰는 다른 프로그램을 끄면 다시 연결됩니다." as UiText,
  },
  "onboarding.failed": {
    ko: "토스에 연결하지 못했어요. 잠시 뒤 다시 시도해 주세요." as UiText,
  },
  "onboarding.ipTitle": { ko: "허용 IP 등록만 하면 끝나요" as UiText },
  "onboarding.ipBody": {
    ko: "이 Mac의 IP를 토스에 한 번만 등록해 주세요." as UiText,
  },
  "onboarding.ipLocation": {
    ko: "WTS > 설정 > Open API > 허용 IP 관리에 아래 IP를 추가하세요." as UiText,
  },
  "onboarding.ipLabel": { ko: "현재 IP" as UiText },
  "onboarding.ipLoading": { ko: "현재 IP를 확인하는 중이에요" as UiText },
  "onboarding.ipUnavailable": {
    ko: "현재 IP를 확인하지 못했어요. 잠시 뒤 다시 확인해요." as UiText,
  },
  "onboarding.copy": { ko: "IP 복사" as UiText },
  "onboarding.copied": { ko: "복사했어요" as UiText },
  "onboarding.copyFailed": {
    ko: "복사하지 못했어요. IP를 직접 옮겨 적어 주세요." as UiText,
  },
  "onboarding.autoRetry": {
    ko: "등록하면 1분 안에 저절로 다음 단계로 넘어가요." as UiText,
  },
} as const;

export type OnboardingMessageKey = keyof typeof catalog;

export const onboardingText = (
  key: OnboardingMessageKey,
  locale: Locale,
): UiText => catalog[key][locale];
