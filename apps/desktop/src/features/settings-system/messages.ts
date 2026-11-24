import type { Locale, UiText } from "../../shared/lib";

const catalog = {
  "system.title": { ko: "시스템" as UiText },
  "system.loading": { ko: "불러오는 중이에요" as UiText },
  "system.loadFailed": {
    ko: "설정을 불러오지 못했어요. 잠시 뒤 다시 열어 주세요." as UiText,
  },
  "system.accountTitle": { ko: "계좌 선택" as UiText },
  "system.accountBody": {
    ko: "종합매매 계좌가 여럿이면 앱이 쓸 계좌를 바꿀 수 있어요." as UiText,
  },
  "system.accountLabel": { ko: "쓸 계좌" as UiText },
  "system.accountNumber": { ko: "계좌 번호" as UiText },
  "system.account1": { ko: "첫 번째 계좌" as UiText },
  "system.account2": { ko: "두 번째 계좌" as UiText },
  "system.account3": { ko: "세 번째 계좌" as UiText },
  "system.account4": { ko: "네 번째 계좌" as UiText },
  "system.account5": { ko: "다섯 번째 계좌" as UiText },
  "system.accountFailed": {
    ko: "계좌를 바꾸지 못했어요. 다시 골라 주세요." as UiText,
  },
  "system.syncTitle": { ko: "토스 자료 갱신" as UiText },
  "system.syncHoldings": {
    ko: "보유 주식과 매수가능금액은 5분마다, 주문 직후에는 바로 다시 받아요." as UiText,
  },
  "system.syncTransactions": {
    ko: "거래내역은 30분마다 최근 14일을 다시 받고, 하루 한 번 전체 기간을 확인해요." as UiText,
  },
  "system.syncSession": {
    ko: "장이 열렸는지는 토스 장 운영 정보로 판단해요." as UiText,
  },
  "system.dataTitle": { ko: "백업과 데이터" as UiText },
  "system.dataBody": {
    ko: "앱은 매일 저절로 백업해요. 필요하면 지금 백업하거나 내보낼 수 있어요." as UiText,
  },
  "system.backup": { ko: "지금 백업" as UiText },
  "system.backedUp": { ko: "백업했어요" as UiText },
  "system.export": { ko: "내보내기" as UiText },
  "system.exported": { ko: "내보냈어요" as UiText },
  "system.exportCancelled": { ko: "내보내기를 취소했어요" as UiText },
  "system.actionFailed": {
    ko: "하지 못했어요. 잠시 뒤 다시 시도해 주세요." as UiText,
  },
  "system.delete": { ko: "전체 삭제" as UiText },
  "system.deleteTitle": { ko: "모든 데이터를 지울까요?" as UiText },
  "system.deleteBody": {
    ko: "저장된 기록과 설정, 토스 키가 모두 지워지고 되돌릴 수 없어요. 다시 쓰려면 토스 키를 새로 붙여 넣어야 해요." as UiText,
  },
  "system.deleteConfirm": { ko: "모두 지우기" as UiText },
  "system.deleteCancel": { ko: "취소" as UiText },
  "system.deleted": { ko: "모두 지웠어요" as UiText },
} as const;

export type SystemMessageKey = keyof typeof catalog;

export const systemText = (key: SystemMessageKey, locale: Locale): UiText =>
  catalog[key][locale];
