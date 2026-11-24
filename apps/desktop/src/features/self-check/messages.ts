import type { Locale, UiText } from "../../shared/lib";

const catalog = {
  "toss-key-invalid": {
    ko: "토스 키가 맞지 않아요. WTS의 Open API 설정에서 키를 다시 복사해 붙여 넣어 주세요." as UiText,
  },
  "toss-key-permission": { ko: "토스 키의 권한을 확인해 주세요." as UiText },
  "toss-account-missing": {
    ko: "토스에서 계좌 상태를 확인해 주세요." as UiText,
  },
  "ip-not-allowed": {
    ko: "이 컴퓨터의 IP가 토스에 등록되어 있지 않아요. WTS의 허용 IP 관리에 추가해 주세요." as UiText,
  },
  "toss-key-shared": {
    ko: "같은 토스 키를 쓰는 다른 프로그램을 끄면 다시 연결돼요." as UiText,
  },
  "claude-missing": {
    ko: "Claude Code를 설치하면 AI 기능이 켜져요." as UiText,
  },
  "claude-signed-out": {
    ko: "Claude Code에 로그인하면 AI 기능이 켜져요." as UiText,
  },
  "order-risk-notice": {
    ko: "이 종목은 토스증권 앱에서 위험 안내를 먼저 확인해야 주문할 수 있어요." as UiText,
  },
  "order-exchange-setting": {
    ko: "토스증권 앱에서 투자자지시 거래소 설정을 마치면 주문할 수 있어요." as UiText,
  },
  "order-dollar-shortage": {
    ko: "달러가 부족해요. 토스증권 앱에서 환전한 뒤 다시 주문해 주세요." as UiText,
  },
  openSettings: { ko: "설정 열기" as UiText },
} as const;

export type SelfCheckMessageKey = keyof typeof catalog;

export const selfCheckText = (
  key: SelfCheckMessageKey,
  locale: Locale,
): UiText => catalog[key][locale];
