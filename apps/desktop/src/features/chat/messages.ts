import type { UiText } from "../../shared/lib";

type Locale = "ko";

const catalog = {
  "chat.emptyTitle": { ko: "무엇이든 물어보세요" as UiText },
  "chat.emptyBody": {
    ko: "보고 있는 종목과 기간을 함께 보내요." as UiText,
  },
  "chat.suggest.portfolio": { ko: "내 보유 종목 지금 어때?" as UiText },
  "chat.suggest.news": { ko: "오늘 시장에서 눈여겨볼 소식 알려 줘" as UiText },
  "chat.suggest.risk": { ko: "비중이 너무 쏠린 종목이 있어?" as UiText },
  "chat.loading": { ko: "지난 대화를 불러오는 중이에요" as UiText },
  "chat.historyFailed": {
    ko: "지난 대화를 불러오지 못했어요." as UiText,
  },
  "chat.retry": { ko: "다시 불러오기" as UiText },
  "chat.assistant": { ko: "AI" as UiText },
  "chat.writing": { ko: "답을 쓰는 중이에요" as UiText },
  "chat.stop": { ko: "멈추기" as UiText },
  "chat.cancelled": { ko: "답을 멈췄어요." as UiText },
  "chat.interrupted": {
    ko: "답이 중간에 끊겼어요. 다시 물어봐 주세요." as UiText,
  },
  "chat.rateLimited": {
    ko: "AI 사용 한도에 닿았어요. 한도가 풀리면 다시 물어봐 주세요." as UiText,
  },
  "chat.failedConnection": {
    ko: "AI와 연결하지 못했어요. 잠시 뒤 다시 물어봐 주세요." as UiText,
  },
  "chat.failedCredential": {
    ko: "AI 로그인이 필요해요. Claude에 다시 로그인한 뒤 물어봐 주세요." as UiText,
  },
  "chat.failedSafety": {
    ko: "안전 점검을 통과하지 못해 답을 멈췄어요." as UiText,
  },
  "chat.failedMessage": {
    ko: "메시지가 비었거나 너무 길어요." as UiText,
  },
  "chat.failedBusy": {
    ko: "다른 AI 작업이 끝나면 다시 물어봐 주세요." as UiText,
  },
  "chat.failedOther": {
    ko: "답을 만들지 못했어요. 다시 물어봐 주세요." as UiText,
  },
  "chat.tool.portfolio": { ko: "보유 종목을 확인하는 중이에요" as UiText },
  "chat.tool.allocation": { ko: "자산 비중을 확인하는 중이에요" as UiText },
  "chat.tool.transactions": { ko: "거래내역을 확인하는 중이에요" as UiText },
  "chat.tool.performance": { ko: "수익률을 확인하는 중이에요" as UiText },
  "chat.tool.quote": { ko: "현재가를 확인하는 중이에요" as UiText },
  "chat.tool.candles": { ko: "가격 흐름을 확인하는 중이에요" as UiText },
  "chat.tool.market": { ko: "시장 상황을 확인하는 중이에요" as UiText },
  "chat.tool.rankings": { ko: "시장 순위를 확인하는 중이에요" as UiText },
  "chat.tool.investorFlow": { ko: "수급을 확인하는 중이에요" as UiText },
  "chat.tool.news": { ko: "모아 둔 뉴스를 찾는 중이에요" as UiText },
  "chat.tool.disclosures": { ko: "공시를 찾는 중이에요" as UiText },
  "chat.tool.article": { ko: "기사 본문을 읽는 중이에요" as UiText },
  "chat.tool.thesis": { ko: "논거 노트를 확인하는 중이에요" as UiText },
  "chat.tool.profile": { ko: "투자 성향을 확인하는 중이에요" as UiText },
  "chat.tool.estimate": { ko: "확률을 따져 보는 중이에요" as UiText },
  "chat.tool.scorecard": { ko: "지난 성적을 확인하는 중이에요" as UiText },
  "chat.tool.webResearch": { ko: "웹에서 찾아보는 중이에요" as UiText },
  "chat.tool.other": { ko: "자료를 확인하는 중이에요" as UiText },
  "chat.tool.appData": { ko: "앱 화면 자료를 확인하는 중이에요" as UiText },
  "chat.tool.appChange": { ko: "앱 설정을 바꾸는 중이에요" as UiText },
  "chat.tool.approval": { ko: "주문 승인을 요청하는 중이에요" as UiText },
  "chat.changed.changeSchedule": { ko: "AI가 작업 일정을 바꿨어요." as UiText },
  "chat.changed.changeProfile": { ko: "AI가 투자 성향을 바꿨어요." as UiText },
  "chat.changed.changeOrderSettings": { ko: "AI가 주문 설정을 바꿨어요." as UiText },
  "chat.changed.addWatchlist": { ko: "AI가 관심 종목을 추가했어요." as UiText },
  "chat.changed.removeWatchlist": { ko: "AI가 관심 종목을 뺐어요." as UiText },
  "chat.changed.skipSignal": { ko: "AI가 신호를 건너뛰었어요." as UiText },
  "chat.changed.changeAccount": { ko: "AI가 계좌를 바꿨어요." as UiText },
  "chat.changed.changePrinciples": { ko: "AI가 투자 원칙을 바꿨어요." as UiText },
  "chat.changed.backup": { ko: "AI가 백업을 만들었어요." as UiText },
  "chat.changed.export": { ko: "AI가 데이터 내보내기 창을 열었어요." as UiText },
  "chat.changed.changeNews": { ko: "AI가 뉴스 출처 설정을 바꿨어요." as UiText },
  "chat.changed.changeContact": { ko: "AI가 SEC 연락처를 바꿨어요." as UiText },
  "chat.changed.changeWake": { ko: "AI가 알림 기준을 바꿨어요." as UiText },
  "chat.changed.changeAliases": { ko: "AI가 종목 별칭을 바꿨어요." as UiText },
  "chat.changed.changeModel": { ko: "AI가 AI 모델 설정을 바꿨어요." as UiText },
  "chat.changed.changeBudget": { ko: "AI가 AI 사용 한도를 바꿨어요." as UiText },
  "chat.changed.approvalRequest": {
    ko: "AI가 주문 승인을 요청했어요. 주문은 신호 카드의 승인 창에서 직접 확인해야 나가요." as UiText,
  },
  "chat.webResearched": { ko: "찾아봤습니다" as UiText },
  "chat.sourcesTitle": { ko: "출처" as UiText },
  "chat.noSources": { ko: "출처 없음" as UiText },
  "chat.source.tool": { ko: "앱 자료" as UiText },
  "chat.source.disclosure": { ko: "공시" as UiText },
  "chat.source.news": { ko: "뉴스" as UiText },
  "chat.source.web": { ko: "미검증 웹 값" as UiText },
  "chat.card.signal": { ko: "신호" as UiText },
  "chat.card.proposal": { ko: "주문 제안" as UiText },
  "chat.card.holding": { ko: "보유 종목" as UiText },
  "chat.card.quote": { ko: "시세" as UiText },
  "chat.card.briefing": { ko: "브리핑" as UiText },
  "chat.card.disclosure": { ko: "공시" as UiText },
  "chat.cardBody": {
    ko: "앱이 확인한 자료로 그린 카드예요. 자세한 내용은 현황에서 볼 수 있어요." as UiText,
  },
  "chat.signalBody": {
    ko: "앱이 계산한 신호예요. 주문은 승인 창에서 본인 확인을 거쳐야 나가요." as UiText,
  },
  "chat.approve": { ko: "주문 승인" as UiText },
  "chat.approveUnavailable": {
    ko: "아직 이 화면에서는 승인할 수 없어요." as UiText,
  },
  "chat.unverified": { ko: "검증 안 됨" as UiText },
  "chat.unverifiedBody": {
    ko: "답에 적힌 숫자 가운데 앱 자료와 맞지 않는 것이 있어요." as UiText,
  },
  "chat.ai.changed": { ko: "AI가 설정을 바꿨어요." as UiText },
  "chat.ai.set_job_schedule": { ko: "AI가 작업 일정을 바꿨어요." as UiText },
  "chat.ai.save_investor_profile": {
    ko: "AI가 투자 성향을 바꿨어요." as UiText,
  },
  "chat.ai.save_order_settings": { ko: "AI가 주문 설정을 바꿨어요." as UiText },
  "chat.ai.add_watchlist": { ko: "AI가 관심 종목을 추가했어요." as UiText },
  "chat.ai.remove_watchlist": { ko: "AI가 관심 종목을 뺐어요." as UiText },
  "chat.ai.skip_signal": { ko: "AI가 신호를 건너뛰었어요." as UiText },
  "chat.ai.select_account": { ko: "AI가 계좌를 바꿨어요." as UiText },
  "chat.ai.set_principles": { ko: "AI가 투자 원칙을 바꿨어요." as UiText },
  "chat.ai.backup_now": { ko: "AI가 백업을 만들었어요." as UiText },
  "chat.ai.export_data": { ko: "AI가 데이터 내보내기를 열었어요." as UiText },
  "chat.ai.set_news_source": {
    ko: "AI가 뉴스 출처 설정을 바꿨어요." as UiText,
  },
  "chat.ai.set_sec_contact": { ko: "AI가 SEC 연락처를 바꿨어요." as UiText },
  "chat.ai.set_wake_criteria": { ko: "AI가 알림 기준을 바꿨어요." as UiText },
  "chat.ai.set_aliases": { ko: "AI가 종목 별칭을 바꿨어요." as UiText },
  "chat.ai.set_task_model": { ko: "AI가 AI 모델 설정을 바꿨어요." as UiText },
  "chat.ai.set_budget_percent": {
    ko: "AI가 AI 사용 한도를 바꿨어요." as UiText,
  },
  "chat.ai.approval": {
    ko: "AI가 주문 승인을 요청했어요. 신호 카드에서 승인 창을 열어 직접 확인해 주세요." as UiText,
  },
  "chat.ai.dismiss": { ko: "확인" as UiText },
} as const;

export type ChatMessageKey = keyof typeof catalog;

export const chatText = (key: ChatMessageKey, locale: Locale): UiText =>
  catalog[key][locale];
