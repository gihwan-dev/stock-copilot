import type { Locale, UiText } from "../../shared/lib";

const catalog = {
  "news.title": { ko: "뉴스·데이터" as UiText },
  "news.loading": { ko: "불러오는 중이에요" as UiText },
  "news.loadFailed": {
    ko: "뉴스·데이터 설정을 불러오지 못했어요. 잠시 뒤 다시 열어 주세요." as UiText,
  },
  "news.saved": { ko: "저장했어요" as UiText },
  "news.saveFailed": { ko: "저장하지 못했어요. 다시 시도해 주세요." as UiText },
  "news.save": { ko: "저장" as UiText },
  "news.sourcesTitle": { ko: "수집 소스" as UiText },
  "news.sourceOn": { ko: "수집" as UiText },
  "news.bodyReading": { ko: "AI 본문 읽기" as UiText },
  "news.bodyLocked": {
    ko: "이 소스는 본문 읽기를 켤 수 없어요" as UiText,
  },
  "news.bodyHint": {
    ko: "끄면 제목과 요약만 AI에 들어가요." as UiText,
  },
  "news.interval": { ko: "수집 주기" as UiText },
  "news.unreachable": {
    ko: "이 기기에서 연결되지 않아요. 잠시 쉬었다가 다시 시도해요. 종목 기사는 Google 뉴스 검색으로 계속 받아요." as UiText,
  },
  "news.source.KindDisclosure": { ko: "KRX KIND 공시" as UiText },
  "news.source.DartDisclosureApi": { ko: "DART 공시 목록" as UiText },
  "news.source.DartDisclosureRss": { ko: "DART 공시 RSS" as UiText },
  "news.source.DartFundamentals": { ko: "DART 재무" as UiText },
  "news.source.Hankyung": { ko: "한국경제" as UiText },
  "news.source.Maekyung": { ko: "매일경제" as UiText },
  "news.source.Yonhap": { ko: "연합뉴스" as UiText },
  "news.source.Edaily": { ko: "이데일리" as UiText },
  "news.source.SegyeFinance": { ko: "세계파이낸스" as UiText },
  "news.source.GoogleNews": { ko: "Google 뉴스" as UiText },
  "news.source.YahooFinance": { ko: "Yahoo Finance" as UiText },
  "news.source.SecEdgar": { ko: "SEC EDGAR" as UiText },
  "news.source.BankOfKorea": { ko: "한국은행" as UiText },
  "news.source.FederalReserve": { ko: "미국 연준" as UiText },
  "news.source.UsTreasury": { ko: "미국 재무부 금리" as UiText },
  "news.source.Fred": { ko: "FRED" as UiText },
  "news.minutes.1": { ko: "1분" as UiText },
  "news.minutes.2": { ko: "2분" as UiText },
  "news.minutes.5": { ko: "5분" as UiText },
  "news.minutes.10": { ko: "10분" as UiText },
  "news.minutes.15": { ko: "15분" as UiText },
  "news.minutes.30": { ko: "30분" as UiText },
  "news.minutes.60": { ko: "60분" as UiText },
  "news.keysTitle": { ko: "선택 키" as UiText },
  "news.key.dart": { ko: "DART 키" as UiText },
  "news.key.fred": { ko: "FRED 키" as UiText },
  "news.keyUnset": { ko: "키를 넣으면 켜집니다" as UiText },
  "news.keySet": { ko: "키가 저장돼 있어요" as UiText },
  "news.keyBenefit.dart": {
    ko: "DART 키가 있으면 공시를 DART에서 받아 정정 표시와 종목 코드까지 정확히 이어요." as UiText,
  },
  "news.keyBenefit.fred": {
    ko: "FRED 키가 있으면 지수·원자재·달러·변동성 일별 값을 직접 받아요." as UiText,
  },
  "news.route.dart.unset": { ko: "지금은 KIND 공시로 받고 있어요." as UiText },
  "news.route.dart.waiting": {
    ko: "DART 첫 수집을 기다리는 중이에요. 그동안 KIND 공시로 받아요." as UiText,
  },
  "news.route.dart.connected": {
    ko: "DART에 연결돼 공시를 DART에서 받고 있어요." as UiText,
  },
  "news.route.dart.failing": {
    ko: "DART에 연결하지 못했어요. 키를 확인해 주세요. 그동안 KIND 공시로 받아요." as UiText,
  },
  "news.route.sec.unset": {
    ko: "연락처가 없어서 미국 공시는 받지 않아요." as UiText,
  },
  "news.route.sec.waiting": {
    ko: "SEC 첫 수집을 기다리는 중이에요." as UiText,
  },
  "news.route.sec.connected": {
    ko: "SEC에 연결돼 미국 보유 종목 공시를 받고 있어요." as UiText,
  },
  "news.route.sec.failing": {
    ko: "SEC에 연결하지 못했어요. 연락처 이메일을 확인해 주세요." as UiText,
  },
  "news.keyInvalid": {
    ko: "키 형식이 맞지 않아요. 다시 복사해 붙여 넣으세요." as UiText,
  },
  "news.secTitle": { ko: "SEC 연락처" as UiText },
  "news.secLabel": { ko: "연락처 이메일" as UiText },
  "news.secHint": {
    ko: "연락처를 넣으면 미국 공시 수집이 켜져요. 비우면 꺼져요." as UiText,
  },
  "news.secInvalid": { ko: "이메일 주소 형식으로 적어 주세요." as UiText },
  "news.wakeTitle": { ko: "AI를 깨우는 기준" as UiText },
  "news.wakeDisclosure": {
    ko: "보유 종목의 긴급·높음 등급 공시는 바로 알려요." as UiText,
  },
  "news.wakeMove": { ko: "보유 종목의 하루 변동" as UiText },
  "news.wakeNews": { ko: "같은 종목 뉴스 (1시간)" as UiText },
  "news.move.3": { ko: "±3 퍼센트 이상" as UiText },
  "news.move.5": { ko: "±5 퍼센트 이상" as UiText },
  "news.move.7": { ko: "±7 퍼센트 이상" as UiText },
  "news.move.10": { ko: "±10 퍼센트 이상" as UiText },
  "news.count.3": { ko: "3건 이상" as UiText },
  "news.count.5": { ko: "5건 이상" as UiText },
  "news.count.10": { ko: "10건 이상" as UiText },
  "news.count.20": { ko: "20건 이상" as UiText },
  "news.aliasTitle": { ko: "종목 약칭" as UiText },
  "news.aliasBody": {
    ko: "뉴스 제목에 줄여 쓰는 이름을 종목 코드와 이어 주세요." as UiText,
  },
  "news.alias": { ko: "약칭" as UiText },
  "news.ticker": { ko: "종목 코드" as UiText },
  "news.aliasRemove": { ko: "삭제" as UiText },
  "news.aliasAdd": { ko: "행 추가" as UiText },
  "news.aliasInvalid": {
    ko: "약칭과 6자리 종목 코드를 모두 적어 주세요." as UiText,
  },
} as const;

export type NewsMessageKey = keyof typeof catalog;

export const newsText = (key: NewsMessageKey, locale: Locale): UiText =>
  catalog[key][locale];
