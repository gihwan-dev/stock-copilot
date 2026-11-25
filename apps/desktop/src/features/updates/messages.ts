import type { Locale, UiText } from "../../shared/lib";

const catalog = {
  readyBar: {
    ko: "새 버전이 준비됐어요. 재시작하면 바로 바뀌어요." as UiText,
  },
  restart: { ko: "재시작" as UiText },
  sheetTitle: { ko: "새 버전으로 다시 시작할까요?" as UiText },
  sheetBody: {
    ko: "앱이 잠깐 닫혔다가 새 버전으로 다시 열려요." as UiText,
  },
  sheetVersion: { ko: "새 버전" as UiText },
  sheetNotes: { ko: "바뀐 점" as UiText },
  sheetConfirm: { ko: "지금 재시작" as UiText },
  sheetLater: { ko: "나중에" as UiText },
  cardTitle: { ko: "앱 업데이트" as UiText },
  currentVersion: { ko: "현재 버전" as UiText },
  check: { ko: "업데이트 확인" as UiText },
  stateOff: {
    ko: "이 빌드에서는 자동 업데이트를 쓰지 않아요." as UiText,
  },
  stateIdle: {
    ko: "앱을 켜 두면 새 버전을 저절로 확인하고 받아 둬요." as UiText,
  },
  stateChecking: { ko: "새 버전을 확인하고 있어요." as UiText },
  stateDownloading: { ko: "새 버전을 받고 있어요." as UiText },
  stateUpToDate: { ko: "최신 버전을 쓰고 있어요." as UiText },
  stateReady: {
    ko: "새 버전이 준비됐어요. 재시작하면 바뀌어요." as UiText,
  },
  stateCheckFailed: {
    ko: "새 버전을 확인하지 못했어요. 인터넷 연결을 확인하고 다시 시도해 주세요." as UiText,
  },
  stateDownloadFailed: {
    ko: "새 버전을 받지 못했어요. 잠시 뒤 다시 시도해 주세요." as UiText,
  },
  stateInstallFailed: {
    ko: "새 버전을 설치하지 못했어요. 앱을 응용 프로그램 폴더로 옮긴 뒤 다시 시도해 주세요." as UiText,
  },
  stateLoading: { ko: "불러오는 중이에요." as UiText },
  stateUnavailable: {
    ko: "업데이트 상태를 불러오지 못했어요." as UiText,
  },
} as const;

export type UpdateMessageKey = keyof typeof catalog;

export const updateText = (key: UpdateMessageKey, locale: Locale): UiText =>
  catalog[key][locale];
