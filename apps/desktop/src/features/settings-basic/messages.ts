import type { Locale, UiText } from "../../shared/lib";

const catalog = {
  "basic.loading": { ko: "불러오는 중이에요" as UiText },
  "basic.advanced": { ko: "고급" as UiText },
  "basic.tossTitle": { ko: "토스 연결" as UiText },
  "basic.toss.connected": { ko: "토스증권에 연결돼 있어요" as UiText },
  "basic.toss.needs-keys": { ko: "토스 키를 넣으면 연결됩니다" as UiText },
  "basic.toss.ip-not-allowed": {
    ko: "이 컴퓨터의 IP가 토스 허용 IP에 없어 연결을 기다리고 있어요" as UiText,
  },
  "basic.toss.invalid-key": {
    ko: "토스 키가 맞지 않습니다. 키를 다시 붙여 넣으세요." as UiText,
  },
  "basic.toss.forbidden": { ko: "토스 키의 권한을 확인하세요." as UiText },
  "basic.toss.account-not-found": {
    ko: "토스에서 계좌 상태를 확인하세요." as UiText,
  },
  "basic.toss.key-in-use": {
    ko: "같은 토스 키를 쓰는 다른 프로그램을 끄면 다시 연결됩니다." as UiText,
  },
  "basic.toss.unknown": {
    ko: "연결 상태를 확인하지 못했어요. 잠시 뒤 다시 열어 주세요." as UiText,
  },
  "basic.clientIdLabel": { ko: "client_id" as UiText },
  "basic.secretLabel": { ko: "secret" as UiText },
  "basic.reconnect": { ko: "키 다시 넣기" as UiText },
  "basic.keyInvalid": {
    ko: "키 형식이 맞지 않아요. 다시 복사해 붙여 넣으세요." as UiText,
  },
  "basic.keyFailed": {
    ko: "토스에 연결하지 못했어요. 잠시 뒤 다시 시도해 주세요." as UiText,
  },
  "basic.profileTitle": { ko: "투자 성향" as UiText },
  "basic.profileFailed": {
    ko: "투자 성향을 불러오지 못했어요. 잠시 뒤 다시 열어 주세요." as UiText,
  },
  "basic.horizonLabel": { ko: "기간" as UiText },
  "basic.horizon.days": { ko: "며칠" as UiText },
  "basic.horizon.weeks-months": { ko: "몇 주~몇 달" as UiText },
  "basic.horizon.half-year-plus": { ko: "반년 이상" as UiText },
  "basic.riskLabel": { ko: "위험 성향" as UiText },
  "basic.risk.conservative": { ko: "보수" as UiText },
  "basic.risk.neutral": { ko: "중립" as UiText },
  "basic.risk.aggressive": { ko: "공격" as UiText },
  "basic.excludeLabel": { ko: "신호에서 뺄 종목 유형" as UiText },
  "basic.exclude.etf": { ko: "ETF" as UiText },
  "basic.exclude.etn": { ko: "ETN" as UiText },
  "basic.exclude.preferred": { ko: "우선주" as UiText },
  "basic.exclude.spac": { ko: "스팩" as UiText },
  "basic.exclude.reit": { ko: "리츠" as UiText },
  "basic.stockCapLabel": { ko: "한 종목 비중 상한" as UiText },
  "basic.stockCap.5": { ko: "총자산의 5 퍼센트" as UiText },
  "basic.stockCap.10": { ko: "총자산의 10 퍼센트" as UiText },
  "basic.stockCap.15": { ko: "총자산의 15 퍼센트" as UiText },
  "basic.stockCap.20": { ko: "총자산의 20 퍼센트" as UiText },
  "basic.stockCap.25": { ko: "총자산의 25 퍼센트" as UiText },
  "basic.stockCap.30": { ko: "총자산의 30 퍼센트" as UiText },
  "basic.sectorCapLabel": { ko: "한 섹터 비중 상한" as UiText },
  "basic.sectorCap.20": { ko: "총자산의 20 퍼센트" as UiText },
  "basic.sectorCap.30": { ko: "총자산의 30 퍼센트" as UiText },
  "basic.sectorCap.40": { ko: "총자산의 40 퍼센트" as UiText },
  "basic.sectorCap.50": { ko: "총자산의 50 퍼센트" as UiText },
  "basic.sectorCap.60": { ko: "총자산의 60 퍼센트" as UiText },
  "basic.profileSave": { ko: "저장" as UiText },
  "basic.profileSaved": { ko: "저장했어요" as UiText },
  "basic.saveFailed": {
    ko: "저장하지 못했어요. 다시 시도해 주세요." as UiText,
  },
  "basic.orderTitle": { ko: "주문 켜기" as UiText },
  "basic.orderLabel": { ko: "주문 켜기" as UiText },
  "basic.orderOff": {
    ko: "꺼져 있으면 신호 카드만 보이고 주문 버튼은 없어요." as UiText,
  },
  "basic.orderOn": {
    ko: "신호 카드에서 승인하면 주문이 나가요." as UiText,
  },
  "basic.orderFailed": {
    ko: "주문 설정을 불러오지 못했어요. 잠시 뒤 다시 열어 주세요." as UiText,
  },
} as const;

export type BasicMessageKey = keyof typeof catalog;

export const basicText = (key: BasicMessageKey, locale: Locale): UiText =>
  catalog[key][locale];
