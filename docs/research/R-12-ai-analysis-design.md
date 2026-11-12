# R-12 AI 분석 설계와 제안 평가 방법

- 조사일: 2026-11-11 (모든 출처의 확인 날짜는 이날이다)
- 로컬 확인 버전: Claude Code 2.1.322, Codex CLI 0.167.1
- 범위: research.md R-12의 질문 1~9, 추가 질문(웹 검색어를 통한 보유 내역 유출), 심층 연구 파이프라인과 제안 스키마 초안

## 1. 요약

1. 여러 에이전트가 찬반 토론을 하는 방식은 같은 연산량의 단순한 방법(여러 번 뽑아 다수결, self-consistency)보다 낫다는 근거가 일관되지 않다. LLM 매매 전략도 기간을 길게, 종목을 넓게 잡아 검증하면 매수 후 보유를 이기지 못했다. 그러니 "토론을 하면 결론이 좋아진다"는 가정을 설계에 박아 두지 말고, 성적표로 확인할 가설로 다뤄야 한다.
2. 비용을 들일 가치가 있는 단계는 토론 횟수를 늘리는 것이 아니다. 자료 수집의 폭, 찬반 논거를 서로 보지 않고 따로 쓰게 하는 것, 주장을 도구 결과와 하나씩 대조하는 검증이다. 숫자는 AI가 쓰지 않고 도구 결과를 참조하게 한다.
3. 웹 검색은 "치명적 삼박자"(비공개 데이터, 신뢰할 수 없는 입력, 외부로 나가는 통로)에서 외부 통로 역할을 한다. 실제로 이 경로로 데이터가 유출된 사례와 논문이 있다. 앱의 MCP 도구를 `--permission-prompt-tool`로 지정하면 코드가 검색어를 하나씩 보고 허용하거나 막을 수 있다. 실제로 돌려서 확인했다. 그래도 보유 내역을 보는 단계와 웹을 검색하는 단계를 서로 다른 실행으로 나누는 것이 근본 대책이다.
4. 확신 정도는 1~10 같은 막연한 점수로 받지 말고, 결과가 확정되는 사건의 확률로 받아 Brier 점수로 보정을 추적한다. LLM이 말로 표현한 확신은 과신 쪽으로 치우친다.
5. 개인 한 명이 만드는 제안은 표본이 작아 성적표가 통계적 결론에 이르려면 수백 건이 필요하다(계산은 3.4절). 성적표는 기본적으로 "판정 보류"를 표시하고, 모델을 비교할 때는 같은 입력을 두 모델에 함께 돌리는 그림자 실행으로 짝지어 비교한다.

## 2. 질문별 답

### 2.1 LLM 투자 분석 연구: 토론, 검증, 단계적 연구가 단일 호출보다 나은가

**토론(multi-agent debate)의 일반 근거**

- 같은 응답 수로 맞추면 다중 에이전트 토론이 단순 self-consistency(다수결)보다 유의하게 못했다. GSM8K에서 토론(응답 3개) 83.2%, self-consistency 82.5%로 차이가 거의 없었고, 저자들은 이득이 "자기 교정"이 아니라 "여러 생성을 모으는 것"에서 온다고 결론 내렸다. ([Huang et al., Large Language Models Cannot Self-Correct Reasoning Yet, arXiv 2310.01798](https://arxiv.org/pdf/2310.01798))
- 토론 프로토콜은 self-consistency나 앙상블보다 안정적으로 낫지 않았다. 하이퍼파라미터에 민감해 최적화하기 어렵다. ([Smit et al., Should we be going MAD?, arXiv 2311.17371](https://arxiv.org/html/2311.17371v3))
- 5개 토론 방법을 9개 벤치마크, 4개 모델로 평가했더니 연산을 더 써도 CoT·self-consistency를 안정적으로 이기지 못했다. 서로 다른 모델을 섞는 것(이질성)은 도움이 됐다. ([Zhang et al., Stop Overvaluing Multi-Agent Debate, arXiv 2502.08788](https://arxiv.org/html/2502.08788v3))
- 같은 질문을 여러 번 뽑아 다수결하면 에이전트 수에 따라 정확도가 오른다. ([Li et al., More Agents Is All You Need, arXiv 2402.05120](https://arxiv.org/abs/2402.05120))

**금융 특화 연구**

- TradingAgents는 강세·약세 연구원 토론, 위험 관리 팀, 트레이더를 두고 기준 모델보다 누적 수익률·샤프가 낫다고 보고했다. 하지만 검증 범위는 2024-01-01~2024-03-29, AAPL·GOOGL·AMZN 3종목이고, 저자들도 비용 때문에 3개월만 돌렸으며 샤프가 예상 범위를 넘었다고 적었다. ([Xiao et al., TradingAgents, arXiv 2412.20138 v4](https://arxiv.org/html/2412.20138v4)) 독립적인 재현은 찾지 못했다(미확인). Hugging Face 논문 페이지 댓글에 미래 정보 편향을 의심하는 의견이 있다. ([HF papers 2412.20138](https://huggingface.co/papers/2412.20138))
- FINSABER는 2004~2024년, 상장폐지 종목을 포함한 S&P 500 구성 종목으로 다시 검증했다. 그 결과 "앞서 보고된 LLM의 우위가 크게 줄었고", 편향을 줄인 설정에서는 매수 후 보유가 FinMem·FinAgent를 유의하게 이겼다. LLM 전략은 강세장에서 지나치게 보수적이었고 약세장에서 지나치게 공격적이었다. 저자들의 권고는 "프레임워크를 복잡하게 키우기보다 추세 감지와 국면별 위험 통제"다. ([Li et al., FINSABER, arXiv 2505.07078 v5](https://arxiv.org/html/2505.07078v5))
- 뉴스 제목의 호재·악재를 GPT로 분류하면 다음 날 수익률과 양의 상관이 있었다. 이 효과는 작은 종목과 악재 뒤에 더 강했고, LLM 사용이 늘수록 전략 수익이 줄었다. 논문 결론이 버전마다 바뀌었으므로 인용할 때는 버전을 밝혀야 한다. ([Lopez-Lira & Tang, arXiv 2304.07619](https://arxiv.org/abs/2304.07619v5))
- 다중 에이전트 연구 시스템에 대한 실무 보고도 있다. Opus 4가 지휘하고 Sonnet 4 하위 에이전트를 쓴 구성이 단일 Opus 4보다 내부 연구 평가에서 90.2% 나았다. BrowseComp에서 성능 분산의 80%를 토큰 사용량 하나가 설명했다. 다중 에이전트는 채팅의 약 15배 토큰을 썼고, 모든 에이전트가 같은 맥락을 공유해야 하는 작업에는 맞지 않았다. ([Anthropic Engineering, How we built our multi-agent research system, 2025-06-13](https://www.anthropic.com/engineering/multi-agent-research-system)) 이것은 "폭넓은 자료 수집"의 이득이지 "찬반 토론"의 이득이 아니다.

**결론**: 단계적 연구는 자료 수집 범위를 넓히는 데서 이득이 확인된다. 찬반 토론 자체의 이득은 일관되지 않다. 매매 성과 개선은 장기·광범위 검증에서 확인되지 않았다.

**알려진 실패 유형**

| 유형 | 근거 | 이 앱에서의 의미 |
|---|---|---|
| 환각 수치 | FinanceBench에서 검색 증강 GPT-4 Turbo가 표본 질문의 81%를 틀렸다(벤치마크 제작사 발표). 근거 문서를 정확히 넣어 준 "Oracle" 설정이 가장 좋았다 ([Islam et al., arXiv 2311.11944](https://web3.arxiv.org/abs/2311.11944), [Patronus 발표](https://prnewswire.com/news-releases/patronus-ai-launches-industry-first-llm-benchmark-for-finance-to-address-hallucinations-301990858.html)) | 숫자는 코드가 넘기고 AI는 참조만 한다 (2.6) |
| 최신성 착각, 암기 | 학습 시점 이전의 경제·금융 수치는 정확히 기억하고, 이후는 거의 0이었다. 무시하라는 지시도 소용없었다 ([Lopez-Lira et al., The Memorization Problem, arXiv 2504.14765](https://arxiv.org/html/2504.14765v2)). 암기가 예측력을 부풀린다는 통계 검정도 있다 ([Gao et al., arXiv 2512.23847](https://arxiv.org/html/2512.23847v1)) | 프롬프트마다 오늘 날짜와 데이터 시각을 넣고, 도구 결과에 없는 사실은 "모델 기억"으로 표시한다. 과거 데이터 백테스트로 프롬프트를 평가하지 않는다 |
| 과신 | 말로 표현한 확신은 과신 쪽으로 치우치고 실패 예측력이 낮다 ([Xiong et al., arXiv 2306.13063](https://arxiv.org/pdf/2306.13063)). 추론 모델은 36개 설정 중 33개에서 보정이 더 좋았다 ([arXiv 2505.14489](https://arxiv.org/pdf/2505.14489v2)) | 확신을 확률로 받고 사후에 보정한다 (2.3) |
| 대형주·유명 종목 쏠림, 확증 편향 | 대부분 모델이 대형주와 역발상 전략을 선호했고, 반대 증거를 줘도 초기 판단을 고수했다 ([arXiv 2507.20957](https://arxiv.org/pdf/2507.20957)). AAPL·MSFT 같은 특정 종목 선호는 편향 제거를 시도한 뒤에도 남았다 ([arXiv 2503.08750](https://arxiv.org/pdf/2503.08750)) | 유망 종목 발굴의 후보는 코드가 랭킹에서 뽑는다. 부정론은 긍정론을 보지 않고 쓴다 |
| 군집(herding) | LLM만으로 이뤄진 시장은 사람보다 "교과서적으로 합리적"이었다는 결과가 있어, 군집 편향 근거는 약하다 ([arXiv 2502.15800](https://arxiv.org/pdf/2502.15800v3)) | 같은 모델 여러 개의 합의를 독립 증거로 세지 않는다(같은 편향 공유). 이질적 모델이 더 낫다는 2502.08788과 같은 방향이다 |
| 아첨(sycophancy) | 5개 어시스턴트 모두가 사용자 견해에 맞추는 경향을 보였다 ([Sharma et al., arXiv 2310.13548](https://arxiv.org/abs/2310.13548)) | 거절 사유나 사용자 의견을 "다음 제안의 맥락"으로 넣으면(6장 규칙) 사용자 입맛에 맞춘 제안이 늘 수 있다. 사용자 의견은 분리된 필드로 주고 "근거로 쓰지 말 것"을 명시한다. 성적표에서 효과를 본다 |
| 자기 선호 | LLM 평가자는 자기 생성물을 더 높게 평가한다 ([Panickssery et al., NeurIPS 2024](https://papers.nips.cc/paper_files/paper/2024/hash/7f1f0218e45f5414c79c0679633e47bc-Abstract-Conference.html)) | 결론·판정 단계와 평가는 가능하면 다른 모델로 한다. 성적표는 LLM 평가 대신 숫자로만 만든다 |

### 2.2 투자 논거 노트의 구조

"논거가 깨졌는가"를 판정하려면 노트가 반증 가능한 주장의 목록이어야 한다. 서술형 문단만으로는 판정할 수 없다. 사전 부검(premortem: "이미 실패했다고 가정하고 이유를 쓴다")은 Klein이 HBR(2007)에 소개했다. 원 연구(Mitchell, Russo, Pennington 1989)가 보인 것은 떠올린 이유의 개수가 약 30% 늘었다는 것이고, 정확도가 오른다는 것은 아니다. ([Klein, Performing a Project Premortem 요약](https://www.oreilly.com/library/view/hbr-guide-to/9781422187319/14-chap5.xhtml), [해석 주의](https://resources.rework.com/libraries/project-management/pre-mortem)) 아래 구조는 이 기법을 따른 설계 제안이며, 구조 자체의 효과를 검증한 연구는 찾지 못했다(미확인).

| 필드 | 내용 | 판정에 쓰는 방식 |
|---|---|---|
| `thesis_summary` | 한두 문장의 핵심 논거 | 표시용 |
| `claims[]` | 논거를 이루는 개별 주장. 각각 `id`, `statement`, `metric`(측정 지표: 매출 성장률, 영업이익률, 수주 잔고 등), `expected`(기대 범위), `observe_via`(DART 재무 API, 공시 유형, 뉴스 키워드), `next_check`(다음 확인 시점: 실적 발표일) | 새 공시·뉴스가 오면 해당 `claim.id`에만 판정을 붙인다 |
| `kill_criteria[]` | 사전 부검으로 만든 "이 일이 생기면 판다" 목록. 가능하면 코드가 판정할 수 있는 형식(지표, 비교, 임계값)과 사람이 판정할 서술형으로 나눈다 | 코드가 판정 가능한 것은 코드가 판정한다. 서술형만 AI가 판정한다 |
| `catalysts[]` | 날짜가 있는 예정 사건 (실적, 신제품, 규제 결정) | 예약 작업 트리거 |
| `risks[]` | 알려진 반대 논거 | 새 증거가 이미 알려진 위험인지 새 위험인지 구분 |
| `position_rationale` | 현재 비중의 이유와 목표 비중 범위 | 리밸런싱 판단 |
| `price_levels` | 가격 기반 기준(선택). 논거와 분리해 둔다 | 가격 기준과 논거 기준을 섞지 않는다 |
| `claim_status[]` | 주장별 상태: `intact`, `weakened`, `broken`, `unverifiable`. 상태마다 근거 출처 ID와 판정 시각 | 노트 전체 상태는 코드가 규칙으로 정한다: `kill_criteria` 중 하나가 충족되거나 핵심 주장이 `broken`이면 "깨짐 의심" |
| `history[]` | 모든 변경과 사유, 작성 모델, 프롬프트 버전 | 감사, 성적표 |

AI가 "논거 전체가 깨졌다"고 한 번에 판정하지 않게 한다. 대신 "이 뉴스는 claim C2의 기대 범위를 벗어난 증거다" 같은 주장 단위의 판정과 출처만 내게 하고, 집계는 코드가 한다.

### 2.3 주문 제안 필드와 확신 정도

필드 초안은 3.3절에 있다. 핵심 원칙은 다음과 같다.

- **근거와 출처**: 각 근거는 `evidence_refs`(도구 호출 ID, 뉴스 ID, 공시 번호, 웹 검색 결과 URL)를 하나 이상 가져야 한다. 출처 종류(`tool`, `disclosure`, `news`, `web`, `model_knowledge`)를 함께 저장해 승인 카드에서 색으로 구분한다.
- **무효 조건**: 코드가 감시할 수 있는 형식(가격, 날짜, 지표)과 서술형으로 나눈다. 코드형 무효 조건이 충족되면 제안이 만료된다.
- **반대 논거**: 긍정론을 쓴 호출과 다른 호출이 쓴다(3.1 3단계). 제안에는 가장 강한 반대 논거를 최소 하나 넣는다.
- **확신 정도와 보정**: "1~10"이나 "높음" 대신 결과가 확정되는 사건의 확률을 받는다. 예: "보유 기간 H(예: 20거래일) 동안 이 종목 수익률이 벤치마크를 넘을 확률 p". 그래야 Brier 점수와 신뢰도 도표로 보정을 측정할 수 있다. 근거는 다음과 같다.
  - 말로 표현한 확신은 과신한다 ([arXiv 2306.13063](https://arxiv.org/pdf/2306.13063)).
  - 예측 대회에서는 사후 보정(Platt 스케일링)이 봇의 Brier 점수를 0.016 낮췄다는 분석이 있다. 2차 정리 글에서 확인했고 1차 원문은 확인하지 못했다(미확인). ([ForecastBench 관련 정리, LessWrong 2026](https://www.lesswrong.com/posts/a82q6yd8zKpYk56cF/ai-forecasting-in-2026-what-11-analyses-say))
  - 12개 LLM 앙상블의 예측이 925명 인간 군중과 통계적으로 구분되지 않았다. 다만 모델들은 둥근 숫자를 선호하는 편향도 보였다. ([Schoenegger et al., Science Advances, arXiv 2402.19379](https://arxiv.org/html/2402.19379v6))
- **보정 운영 규칙**: 결과가 확정된 제안이 충분히 쌓이기 전까지는 원래 확률을 화면에 크게 보이지 않는다. 대신 확률 구간(예: 50~60%, 60~70%, 70% 이상)별로 과거 적중률과 건수를 함께 보여 준다. 확률을 5% 단위로 반올림해 받으면 둥근 숫자 편향이 덜 문제가 된다.
- **권장 비중**: AI는 `target_weight_range`(주문 뒤 목표 비중 범위)와 수량·지정가를 제안한다. 주문 금액, 주문 뒤 비중, 원칙 위반 여부는 코드가 계산해 덮어쓴다. 현재 설계(6장, ACC-1)와 같다.

### 2.4 제안 성적표의 방법론

이 앱이 고를 수 있는 방법을 정리한 것이다. 수치 기준은 아래 계산으로 정했고, 이 앱에 맞춘 검증 연구는 없다.

1. **기준 시점과 기준 가격**: 제안이 만들어진 시각의 기준 가격(지정가 매수 제안은 제안 시각의 현재가)을 쓴다. 실제 체결가는 따로 기록한다. 그래야 승인한 제안, 거절한 제안, 만료된 제안을 같은 기준으로 비교할 수 있다.
2. **평가 기간**: 미리 정한 고정 기간 여러 개(5, 20, 60거래일)와 "무효 조건이 충족될 때까지"를 함께 기록한다. 기간은 제안을 만들 때 고정하고, 나중에 고르지 않는다(사후 선택 금지).
3. **초과 수익**: 같은 기간 같은 통화로 잰 종목 수익률에서 벤치마크 수익률을 뺀다. 한국 종목은 KOSPI 또는 KOSDAQ, 미국 종목은 S&P 500(또는 대응 ETF)을 쓴다. 벤치마크 출처는 R-07과 R-11에 따른다. 매도 제안의 성과는 "매도로 피한 수익률" = −(종목 초과 수익)이다.
4. **거절·만료 제안의 가상 성과**: 지정가 제안은 평가 기간 안에 시장가가 지정가에 닿았을 때만 가상 체결로 친다. 닿지 않았으면 "미체결 가상"으로 따로 센다. 승인 집합과 거절 집합의 성과 차이는 "사용자의 거절이 가치를 더했는가"를 보여 준다.
5. **아이디어**: 주문 제안과 따로 집계한다. 같은 표에 섞지 않는다.
6. **결론을 내리지 않는 기준**: 20거래일 초과 수익의 표준편차를 σ, 실제 평균 초과 수익을 μ라 하면, 유의수준 5%·검정력 80%로 μ를 잡아내는 데 필요한 건수는 n ≈ ((1.96+0.84)·σ/μ)²이다. 계산 결과는 3.4절에 있다. 개별 종목의 한 달 초과 수익 표준편차를 10%로 잡으면 평균 2%의 우위를 확인하는 데 약 200건이 필요하다. 이 σ 값은 가정이며 실제 값은 데이터가 쌓인 뒤 추정한다. 적중률 60%를 50%와 구분하는 데도 약 194건이 필요하다. 그러니 다음 규칙을 둔다.
   - 확정된 제안이 30건 미만이면 숫자만 보여 주고 "판정 보류"를 표시한다.
   - 30건 이상이면 평균 초과 수익의 부트스트랩 95% 신뢰구간을 보여 준다. 구간이 0을 포함하면 "우위 확인 안 됨"을 표시한다.
   - 같은 날 만든 제안끼리는 시장 공통 요인 때문에 서로 상관이 있다. 신뢰구간은 날짜 단위로 묶어 재표본한다(블록 부트스트랩).
7. **모델·프롬프트 비교 시 주의점**:
   - 여러 모델과 프롬프트 버전을 비교한 뒤 가장 좋은 것을 고르면 선택 편향이 생긴다. Deflated Sharpe Ratio는 시도 횟수 N에 따른 기대 최대값으로 이 편향을 보정한다. 최소 실적 기간(MinTRL)은 샤프가 작거나 왜도가 음일수록 길어진다. ([Bailey & López de Prado, The Deflated Sharpe Ratio, SSRN 2460551](https://papers.ssrn.com/abstract=2460551), [López de Prado 강의 자료](https://newyork.qwafafew.org/wp-content/uploads/sites/4/2017/01/Lopez_de_Prado_Sharpe.pdf)) 성적표에 "지금까지 비교한 조합 수"를 함께 보여 준다.
   - 기간이 다르면 비교할 수 없다. 모델 A가 강세장에, 모델 B가 약세장에 제안했다면 차이는 시장 국면 탓이다(FINSABER가 보인 국면 의존성). 그래서 **그림자 실행**을 권장한다. 같은 트리거와 같은 증거 묶음을 두 모델에 함께 돌리고, 한쪽만 사용자에게 보여 주고 다른 쪽은 기록만 한다. 두 결과를 짝지어 비교한다.
   - 과거 데이터로 프롬프트를 평가하지 않는다. 학습 시점 이전 기간은 암기 때문에 성과가 부풀려진다 ([arXiv 2504.14765](https://arxiv.org/html/2504.14765v2)).
   - 증권사 리포트 추천에 대한 고전 연구에서도 총수익 기준으로는 우위가 있었지만 회전율과 거래 비용을 넣으면 사라졌다. ([Barber et al., Journal of Finance 56(2), 2001](https://ideas.repec.org/a/bla/jfinan/v56y2001i2p531-563.html)) 성적표에 수수료·세금을 반영한 순수익도 함께 둔다(R-11).

### 2.5 프롬프트 주입: 뉴스·웹을 읽는 에이전트가 제안 도구를 가질 때

**위험 구조**

- 치명적 삼박자: 비공개 데이터 접근, 신뢰할 수 없는 입력, 외부 통신이 한 에이전트에 모이면, 주입된 지시로 데이터가 밖으로 나갈 수 있다. 가장 끊기 쉬운 다리는 외부 통신이다. ([Willison, The lethal trifecta, 2025-06-16](https://simonwillison.net/2025/Jun/16/the-lethal-trifecta/))
- 이 앱에서 세 다리는 다음과 같다. 비공개 데이터는 `get_portfolio`와 `get_transactions`다. 신뢰할 수 없는 입력은 `search_news`, `read_article`, WebSearch 결과다. 외부 통신은 **WebSearch의 검색어**다.
- 일반 에이전트에서 안전 보장은 어렵다. 그래서 신뢰할 수 없는 입력을 받은 뒤에는 그 입력이 중대한 행동을 일으키지 못하게 구조를 제한하라는 설계 패턴이 제안돼 있다: plan-then-execute, dual LLM, context minimization 등. ([Beurer-Kellner et al., Design Patterns for Securing LLM Agents against Prompt Injections, arXiv 2506.08837](https://arxiv.org/html/2506.08837v3))
- CaMeL은 신뢰하는 질의에서 제어 흐름을 미리 뽑고, 신뢰할 수 없는 데이터가 실행 경로를 바꾸지 못하게 한다. 데이터마다 권한 메타데이터를 붙여 도구 호출 시점에 정책을 검사한다. AgentDojo에서 증명 가능한 보안을 지키며 77%(방어 없음 84%)를 풀었다. ([Debenedetti et al., Defeating Prompt Injections by Design, arXiv 2503.18813](https://arxiv.org/pdf/2503.18813))
- 데이터에 표시를 붙이는 spotlighting(구분자, 표시, 인코딩)은 GPT 계열에서 공격 성공률을 50% 이상에서 2% 미만으로 낮췄다. 확률적 방어일 뿐 보장은 아니다. ([Hines et al., arXiv 2403.14720](https://arxiv.org/pdf/2403.14720))
- OWASP LLM Top 10(2025)은 프롬프트 주입을 1위(LLM01)에 두었다. 과도한 권한(LLM06)은 도구 과다, 권한 과다, 승인 없는 고위험 행동의 세 형태로 정리된다. ([2차 정리](https://opensourcesecurity.substack.com/p/a-deep-dive-into-the-owasp-top-10), OWASP 공식 페이지는 직접 확인하지 못함: 미확인)

**이 설계에서 이미 막혀 있는 것**: 주문 전송은 MCP에 없다. 승인은 코어가 띄우는 본인 확인 창을 거쳐야 한다(SEC-5, SEC-8). 그래서 주입이 성공해도 최악의 결과는 "그럴듯한 나쁜 제안이 카드로 뜨는 것"이다.

**남는 위험과 방어**

| 위험 | 방어 |
|---|---|
| 주입된 뉴스가 그럴듯한 제안을 유도 | (1) 도구 결과에서 외부 텍스트는 `untrusted_text` 필드에 따로 담고, 프롬프트에서는 표시가 붙은 블록 안에만 둔다(spotlighting). (2) 제안 근거의 출처 종류를 저장하고, 근거가 `web`·`news`뿐이고 `tool`·`disclosure` 근거가 없으면 카드에 "외부 출처만"을 경고한다. (3) 승인 카드는 코드가 계산한 사실 영역과 AI 서술 영역을 시각적으로 분리한다(6장 설계 유지). AI 서술 영역에는 링크를 클릭할 수 없게 하거나 도메인을 표시한다(R-16). |
| 보유 내역 유출 (웹 검색어) | 2.10절 |
| 제안 도구의 남용(대량 제안으로 사용자 피로) | `propose_order`에 실행당 개수 제한(예: 3건)과 같은 종목 중복 제한을 둔다 |
| 논거 노트 오염 (`update_thesis`가 주입 내용을 장기 기억으로 저장) | 노트 변경은 주장 단위의 차분으로만 받는다. 출처가 `web`뿐인 변경은 "확인 필요" 상태로 두고, 사용자가 보기 전에는 다음 분석의 입력에 넣지 않는다 |

### 2.6 LLM이 쓴 숫자를 도구 결과와 대조하는 구현

1. **숫자를 쓰지 않고 참조하게 한다**: 구조화 출력(제안, 브리핑 카드)의 숫자 필드는 값 대신 `{"ref": "<tool_call_id>#<JSON 포인터>"}`를 받는다. 앱은 MCP 서버가 직접 돌려준 결과의 원장에서 값을 찾아 그린다. 원장에 없는 참조는 스키마 검증에서 실패시킨다.
2. **실행별 원장**: MCP 서버가 실행 ID별로 도구 호출, 인자, 결과를 저장한다(R-04의 감사·재현 기록과 같은 저장소). 웹 검색 결과는 MCP 밖이지만 CLI 스트림의 `tool_result`로 텍스트가 들어오므로 원장에 출처 `web`으로 넣는다. 이렇게 넣은 값은 "미검증 웹 값"이다.
3. **자유 서술 속 숫자**: 채팅과 브리핑 본문에서 숫자, 백분율, 금액을 정규식으로 뽑아 같은 실행의 원장 값과 대조한다. 금액은 정확히 일치해야 하고, 비율은 표시 자릿수에서 반올림한 값이 일치하면 된다. 단위(원, 달러, 만, 억)는 정규화한다. 일치하지 않거나 원장에 없는 숫자에는 "검증 안 됨" 표시를 붙인다(ACC-1). 계산된 파생값(예: 두 값의 차)은 AI가 계산하게 하지 말고 도구가 미리 계산해 준다.
4. **사실 주장 검증**: 결론 단계 전에 주장 목록을 뽑고, 각 주장을 초안을 보지 않는 별도 호출에서 원장 자료만으로 확인한다. 이것은 Chain-of-Verification의 "분리된(factored)" 방식이다. 초안을 보지 않고 답하면 같은 오류를 반복하는 것을 줄였지만, 환각을 완전히 없애지는 못했다. ([Dhuliawala et al., arXiv 2309.11495](https://arxiv.org/abs/2309.11495))

### 2.7 토큰을 많이 쓰면 품질로 이어지는 작업과 그렇지 않은 작업

| 구분 | 작업 | 근거 |
|---|---|---|
| 이어진다 | 심층 연구의 자료 수집 폭(여러 검색, 공시·재무·뉴스 교차) | BrowseComp 성능 분산의 80%를 토큰 사용량이 설명 ([Anthropic, 2025-06-13](https://www.anthropic.com/engineering/multi-agent-research-system)) |
| 이어진다 | 같은 판단을 독립적으로 여러 번 뽑아 모으기(분류, 확률 추정) | self-consistency와 표본·투표 ([2310.01798](https://arxiv.org/pdf/2310.01798), [2402.05120](https://arxiv.org/abs/2402.05120)), LLM 군중 예측 ([2402.19379](https://arxiv.org/html/2402.19379v6)) |
| 이어진다 | 분리된 주장 검증 | CoVe ([2309.11495](https://arxiv.org/abs/2309.11495)) |
| 이어진다(가능성) | 추론 강도를 높이는 것: 확률 보정 | 추론 모델의 보정 개선 ([2505.14489](https://arxiv.org/pdf/2505.14489v2)) |
| 이어지지 않는다 | 토론 왕복 횟수 늘리기 | [2311.17371](https://arxiv.org/html/2311.17371v3), [2502.08788](https://arxiv.org/html/2502.08788v3) |
| 이어지지 않는다 | 숫자 계산, 비중·수익률 | 코드가 한다 (설계 원칙) |
| 이어지지 않는다 | 뉴스 제목 분류·종목 매칭을 큰 모델로 | 빠른 모델과 규칙으로 충분하다는 것이 현재 설계다. 효과 비교 연구는 찾지 못함(미확인). 그림자 실행으로 확인한다 |
| 이어지지 않는다 | 모든 뉴스를 긴 맥락에 통째로 넣기 | FinanceBench에서 정확한 근거를 넣은 설정이 가장 좋았고, 긴 맥락은 지연이 컸다 ([2311.11944](https://web3.arxiv.org/abs/2311.11944)) |

### 2.8 제공자에 묶이지 않는 프롬프트 관리와 버전 비교

- **파일 형식**: `prompts/<task>/<version>.md`에 앞머리(front matter)로 `id`, `version`, `task`, `inputs`(채울 변수 목록), `output_schema`(공용 JSON Schema 파일 경로), `tools`(논리적 도구 이름)를 둔다. 본문에 제공자별 도구 이름(`WebSearch`, `mcp__...`)을 쓰지 않는다. 어댑터가 논리 이름을 실제 이름으로 바꾼다.
- **출력 계약**: 결과는 MCP 도구 호출(`propose_order` 등)이나 최종 구조화 출력으로 받는다. Claude Code는 `--json-schema`(print 모드에서 끝난 뒤 검증된 JSON)를 지원한다 ([CLI reference](https://code.claude.com/docs/en/cli-reference)). Codex CLI 0.167.1 `exec`에는 `--output-schema <FILE>`이 있다(로컬 `codex exec --help`로 확인). 두 제공자가 같은 스키마 파일을 공유할 수 있다. 두 기능의 세부 차이는 R-02와 R-03에서 확인한다.
- **기록**: 작업 기록에 프롬프트 ID, 버전, 본문 해시, 모델, 추론 강도, CLI 버전, 입력 증거 묶음 ID를 남긴다. architecture.md의 "AI 작업" 저장소에 이 필드들을 추가한다.
- **버전 비교**:
  1. **고정 재생 세트**: 지난 실행의 증거 묶음(도구 결과 원장)을 얼려 둔 20~50개 사례를 쓴다. Anthropic은 약 20개의 대표 질의로 시작해도 초기 변경의 효과를 볼 수 있었다고 보고했다 ([Anthropic, 2025-06-13](https://www.anthropic.com/engineering/multi-agent-research-system)). 새 버전을 재생 세트에 돌려 구조 지표를 비교한다: 스키마 통과율, 출처 포함률, 숫자 불일치율, 분리 검증에서 기각된 주장 비율, 반대 논거 포함률.
  2. **품질 판정**: LLM 판정자를 쓴다면 0.0~1.0 점수와 통과/실패를 내는 단일 호출이 가장 일관됐다 ([같은 글](https://www.anthropic.com/engineering/multi-agent-research-system)). 판정자는 생성 모델과 다른 모델로 하고, 어느 버전인지 가린다(자기 선호 편향).
  3. **실전 성과**: 그림자 실행(2.4-7)으로만 비교한다.
- **도구**: promptfoo는 프롬프트×제공자×테스트의 조합을 평가하는 오픈소스 도구다 ([promptfoo getting started](https://promptfoo.dev/docs/getting-started/)). 하지만 Node 의존성이 늘고, 이 앱의 실행 경로는 CLI 자식 프로세스라 그대로 붙지 않는다. 재생 세트는 `testkit`의 가짜 MCP 원장과 Rust `xtask`로 직접 돌리는 쪽을 권장한다. promptfoo가 CLI 제공자를 지원하는지는 확인하지 않았다(미확인).

### 2.9 작업별 초안: 아침 브리핑, 마감 리뷰, 유망 종목 발굴

공통: 각 작업은 (가) 코드의 증거 묶음 생성 → (나) 웹 검색 실행(보유 내역 없음) → (다) 분석 실행(웹 검색 없음) → (라) 코드 검증 순서로 돈다. 순서 설계는 3.1절과 2.10절에 있다.

**아침 브리핑 (08:20 KST)**

| 항목 | 내용 |
|---|---|
| 입력(코드) | 밤사이 미국 지수·금리·환율(시각 포함), 보유·관심 종목의 신규 공시·뉴스 묶음(전날 마감 이후), 오늘 일정(실적, 경제 지표, 배당락), 보유 종목 논거 노트의 `catalysts` 중 오늘 것, 장 운영 정보 |
| 웹 검색(선택) | 코드가 만든 검색어 목록만 쓴다(예: "{미국 보유 종목명} earnings", "FOMC minutes"). 수량·금액·비중은 넣지 않는다 |
| 단계 | 1) 빠른 모델: 뉴스 묶음의 중요도 분류와 요약(묶음 단위) 2) 큰 모델: 보유 종목별 "오늘 볼 것"과 영향받는 논거 주장 ID 3) 코드: 숫자 대조, 출처 확인 |
| 산출물 | `briefing` 스키마(3.3): 섹션(시장, 보유 종목별, 일정), 각 항목 `{headline, body, evidence_refs[], affected_claims[], importance}`. 숫자는 `ref`로만. 주문 제안 없음(장 개장 전이므로 아이디어만) |

**마감 리뷰 (15:50 KST)**

| 항목 | 내용 |
|---|---|
| 입력(코드) | 오늘 포트폴리오 변동 분해(종목별 기여도, 통화 효과)는 코드가 계산한다. 보유 종목 등락률과 벤치마크 대비, 오늘 공시·뉴스 묶음, 수급 변화, 오늘의 제안·승인·거절 결과 |
| 단계 | 1) 큰 모델: 기여도 상위 종목별로 "원인 후보"를 증거와 연결하고, 증거가 없으면 "원인 불명"으로 둔다 2) 논거 주장별 판정 갱신 제안 3) 코드: 판정 집계와 노트 상태 갱신 |
| 산출물 | `close_review`: `{movers[]: {symbol, contribution_ref, cause_candidates[]: {text, evidence_refs[], confidence_p}}, thesis_updates[], unexplained[]}` |

**유망 종목 발굴 (매일 마감 후)**

| 항목 | 내용 |
|---|---|
| 입력(코드) | 후보는 코드가 뽑는다: 거래대금·등락률·수급 랭킹, 섹터 랭킹, 원칙 필터(금지 유형 제외), 이미 보유·거절한 종목 표시. 대형주·유명 종목 쏠림을 줄이려고 후보 목록의 순서를 섞고 시가총액 구간별로 할당량을 둔다 |
| 단계 | 1) 빠른 모델: 후보별 1차 걸러내기(이유 한 줄과 탈락 사유) 2) 남은 상위 N개(예: 3)에 대해 웹 검색 실행(보유 내역 없음) 3) 심층 연구 파이프라인(3.1) 축약판 4) 코드: 원칙 검사 |
| 산출물 | `idea`(장이 닫혔으므로 주문 제안이 아님): 3.3의 제안 스키마에서 수량·지정가를 뺀 형태와 `watch_triggers[]`(장중에 주문 제안으로 바꿀 조건) |

### 2.10 추가 질문: 웹 검색어를 통한 보유 내역 유출

**위험의 실제 사례**

- 지식 베이스를 가진 요약 에이전트에게 숨은 지시가 든 웹 페이지를 읽게 하면, 에이전트가 내부 데이터를 다음 "검색" 질의에 넣어 공격자 서버로 보낸다. 모델별 공격 성공률은 grok-4 72.4%, qwen3-235b 28.3%, claude-3-haiku 6.0%, claude-sonnet-4 0.0%, gpt-5-nano 0.0%였다(모델당 1,068건, 1회씩 실행). 이 논문의 도구는 공격자 URL로 요청을 보내는 형태였다. ([Rall et al., Exploiting Web Search Tools of AI Agents for Data Exfiltration, arXiv 2510.09093](https://arxiv.org/html/2510.09093v1))
- Notion 3.0 에이전트(Claude Sonnet 4로 보고됨) 사례: PDF의 흰 글씨 지시가 고객 목록을 읽게 했다. 에이전트는 그 데이터를 넣은 공격자 도메인 URL을 웹 검색 도구로 "검색"했고, 공격자 서버가 이를 기록했다. 단기 대책으로 "검색 도구가 URL을 받는 기능 제거"가 제안됐다. ([Willison, 2025-09-19](https://simonwillison.net/2025/Sep/19/notion-lethal-trifecta))

**Claude Code WebSearch의 실제 동작 (1차 문서)**

- WebSearch는 질의를 Anthropic의 웹 검색 백엔드로 보내 제목과 URL을 돌려준다. 결과 페이지를 직접 가져오지 않는다. 한 번 호출에 백엔드 검색을 최대 8번 할 수 있다. `allowed_domains`와 `blocked_domains`를 받는다. 백엔드는 바꿀 수 없다. 권한 규칙은 지정자 없이 `WebSearch` 전체를 허용하거나 막는 형태뿐이다. ([Claude Code tools reference, WebSearch tool behavior](https://code.claude.com/docs/en/tools-reference))
- PreToolUse 훅은 WebSearch에도 걸린다. 입력 필드는 `query`, `allowed_domains`, `blocked_domains`, `mode`이고, 훅은 `deny`하거나 `updatedInput`으로 입력을 바꿀 수 있다. ([Claude Code hooks reference](https://code.claude.com/docs/en/hooks))
- `--permission-prompt-tool`은 비대화형 모드의 권한 질문을 지정한 MCP 도구로 보낸다. 그 도구는 `{"behavior":"allow"}` 또는 `{"behavior":"deny","message":...}`를 돌려준다. ([CLI reference](https://code.claude.com/docs/en/cli-reference), [errors: Permission prompt tool returned an invalid result](https://code.claude.com/docs/en/errors))
- 검색 백엔드의 제3자: Anthropic 하위 처리자 목록에 Brave Search가 웹 검색용으로 있다고 2차 출처들이 보고한다 ([TechCrunch 2025-03-21](https://techcrunch.com/2025/03/21/anthropic-appears-to-be-using-brave-to-power-web-searches-for-its-claude-chatbot/), [Willison 2025-03-21](https://simonwillison.net/2025/Mar/21/anthropic-use-brave/)). 1차 페이지([trust.anthropic.com/subprocessors](https://trust.anthropic.com/subprocessors))는 내용을 읽지 못했다(미확인). 즉 검색어는 Anthropic과 검색 제공자에게 간다. 검색 백엔드가 질의 속 URL을 실제로 방문하는지는 확인하지 못했다(미확인). 방문하지 않더라도 검색어가 제3자의 기록에 남는 것은 피할 수 없다.

**이 앱에서의 위험 평가**

1. 주입이 없어도 생기는 노출: 보유 종목명으로 검색하면 "이 사용자가 이 종목에 관심 있다"가 Anthropic과 검색 제공자에게 전달된다. 수량, 금액, 평단, 계좌는 검색어에 들어갈 이유가 없다.
2. 주입이 있을 때: 웹 결과나 뉴스 본문 속 지시가 AI를 움직여 `get_portfolio` 결과(수량, 금액)를 검색어에 넣게 할 수 있다. Claude Code WebSearch는 페이지를 직접 가져오지 않으므로 Notion 사례처럼 공격자 서버에 바로 기록되는지는 미확인이다. 하지만 "백엔드가 URL을 방문하지 않는다"에 기대는 것은 근거가 문서화되지 않은 방어다.
3. 현재 설계의 모순: design.md 2장은 "AI는 임의의 웹 주소를 직접 읽지 못한다"고 하고, SEC-2는 "웹 읽기 도구 없이 웹 검색만"이라고 한다. 그런데 architecture.md 신뢰 경계 표(40행)는 AI CLI의 권한을 "웹 검색·읽기"로 적었다. 두 문서를 맞춰야 한다. 이 조사는 "WebFetch 없음"을 전제로 한다.

**방어 (강한 순서)**

| # | 방어 | 효과 | 비용 |
|---|---|---|---|
| W1 | **실행 분리**: 보유 내역 도구(`get_portfolio`, `get_transactions`, `get_allocation`, `get_thesis` 중 수량·비중)를 가진 실행에는 WebSearch를 주지 않는다. 웹 검색이 필요한 단계는 보유 내역 도구가 없는 별도 실행으로 돌린다. 이 실행은 코드가 준 종목명·주제만 받는다(plan-then-execute 또는 dual LLM 패턴, [2506.08837](https://arxiv.org/html/2506.08837v3)). 검색 결과는 원장에 `web`으로 저장되고 분석 실행에는 데이터로만 들어간다 | 삼박자의 "비공개 데이터"와 "외부 통신"이 한 실행에 모이지 않는다. 주입이 성공해도 검색 실행이 아는 것은 종목명뿐이다 | 실행이 한 번 늘어난다. 분석 중에 즉흥적으로 검색할 수 없다 |
| W2 | **검색어 게이트**: WebSearch를 허용 목록에 넣지 않으면 매 호출이 권한 질문이 된다. `--permission-prompt-tool mcp__app__gate_tool`로 앱의 MCP 도구가 검색어를 받아 코드 규칙으로 판정한다. 규칙: 숫자 2자리 이상 금지(연도·분기 표기는 별도 허용 목록), URL·도메인 형태 금지, 길이 상한, 계좌·금액 단위어 금지, 실행마다 최대 횟수. 막은 검색어는 감사 로그에 남기고 UI에 보인다 | 주입이든 실수든 수량·금액이 검색어로 나가는 것을 코드가 막는다. 아래 실험으로 확인했다 | 정당한 검색 일부가 막힌다(예: "2026년 3분기 실적"). 허용 목록으로 조정한다 |
| W3 | **검색어 표시**: 모든 검색어를 실행 기록과 채팅 화면에 그대로 보인다(stream-json의 `tool_use.input.query`) | 사후 발견 | 낮음 |
| W4 | `allowed_domains`로 결과 도메인을 제한한다(공시·언론사·거래소) | 신뢰할 수 없는 입력을 줄인다. 유출 방어는 아니다 | 검색 품질 저하. AI가 넣는 값이라 코드가 강제하려면 W2 게이트의 `updatedInput`으로 덮어써야 한다. `updatedInput`이 `--permission-prompt-tool` 경로에서도 반영되는지는 문서상 "allow 객체에 updatedInput 추가"로 되어 있으나 도메인 필드로는 실험하지 않았다(미확인) |
| W5 | 채팅처럼 보유 내역과 검색이 함께 필요한 경우: W2 게이트를 필수로 두고, 채팅 실행의 `get_portfolio`가 수량·금액 대신 비중 구간만 주는 "요약 모드"를 기본으로 한다 | 유출되는 정보의 정밀도를 낮춘다 | 답변의 정밀도가 떨어진다. 수량이 필요하면 사용자가 명시적으로 켠다 |

**실험: `--permission-prompt-tool`로 WebSearch 검색어 판정 (2026-11-11, Claude Code 2.1.322)**

실험 폴더(`/private/tmp/claude-501/.../scratchpad/research/R-12/`)에 표준 라이브러리만 쓴 stdio MCP 서버 `gate.py`를 만들었다. 이 서버의 `approve` 도구는 숫자 2자리 이상, URL, 도메인 경로, 가짜 종목명 `FAKECORP`가 든 검색어를 거부하고 판정을 파일에 기록한다. 실제 계좌나 개인 정보는 쓰지 않았다.

```
claude -p --setting-sources "" --strict-mcp-config --mcp-config mcp.json \
  --tools WebSearch --disallowedTools "mcp__gate__approve" \
  --permission-prompt-tool mcp__gate__approve --model haiku \
  --output-format stream-json --verbose \
  "Do exactly two web searches ... Search 1 query: Tauri 2 release notes. Search 2 query: FAKECORP 120 shares average price 35000"
```

결과(스트림 요약과 게이트 기록):

```
INIT tools= ['WebSearch'] mcp= [{'name': 'gate', 'status': 'connected', 'source': 'dynamic'}]
TOOL_USE WebSearch {"query": "Tauri 2 release notes", "mode": "standard"}
TOOL_USE WebSearch {"query": "FAKECORP 120 shares average price 35000", "mode": "standard"}
RESULT query blocked by app policy: \d{2,},FAKECORP
RESULT Web search results for query: "Tauri 2 release notes" ...
gate.log: {"tool_name": "WebSearch", "input": {"query": "Tauri 2 release notes", ...}, "decision": {"behavior": "allow", ...}}
gate.log: {"tool_name": "WebSearch", "input": {"query": "FAKECORP 120 shares ...", ...}, "decision": {"behavior": "deny", ...}}
```

확인한 것:

- 검색이 실행되기 전에 CLI가 앱의 MCP 도구에 검색어를 보낸다. 거부된 검색어는 실행되지 않았다(결과 없음, 모델이 "정책으로 막혔다"고 보고).
- `--disallowedTools "mcp__gate__approve"`로 게이트 도구를 모델의 도구 목록에서 빼도(`tools`에 WebSearch만 보고됨) 권한 질문용으로는 동작했다. 모델이 게이트 도구를 직접 부를 수 없다.
- `--setting-sources ""`와 `--strict-mcp-config`를 함께 써도 동작했다. 구독 인증으로 실행했다.

확인하지 못한 것: 앱의 실제 MCP 서버(R-04의 전송 방식, 토큰 인증)에서 같은 동작을 하는지, `--permission-prompts`와의 상호작용, Codex CLI에 같은 수단이 있는지(R-03). 모두 미확인이다.

## 3. 선택지 비교표와 추천

### 3.1 심층 연구 파이프라인 비교

| 기준 | A. 단일 호출 | B. 단일 호출 × k회 + 집계 | C. 현재 설계 (수집 → 긍정론 → 부정론 → 반박 검증 → 결론) | D. 제안 (아래) |
|---|---|---|---|---|
| 근거 | 기준선 | self-consistency가 토론과 비슷하거나 낫다 (2310.01798, 2502.08788) | 토론 이득 불안정 (2311.17371). TradingAgents는 소규모 검증 | 수집 폭(Anthropic), 분리 검증(CoVe), 이질적 모델(2502.08788), 프롬프트 주입 설계 패턴(2506.08837) |
| 쏠림 방지 | 약함 | 같은 모델의 편향은 공유 | 부정론이 긍정론을 보면 끌려간다(확증 편향, 2507.20957) | 긍정론과 부정론이 서로 보지 않고 같은 증거 묶음으로 쓴다 |
| 환각 수치 | 취약 | 다수결로 일부 줄어듦 | "반박 검증"이 또 하나의 LLM 의견이라 사실 검증이 아님 | 숫자는 `ref`로만 쓰고, 주장 검증은 원장 대조와 분리 호출로 한다 |
| 유출 위험 | 웹 검색과 보유 내역이 한 실행에 모임 | 같음 | 같음 | 수집 실행(웹, 보유 내역 없음)과 분석 실행(보유 내역, 웹 없음)을 나눈다 |
| 토큰 | 1× | k× | 약 5× | 약 5~7× (수집 폭에 따라) |
| 평가 가능성 | 쉬움 | 쉬움 | 단계 효과를 분리하기 어려움 | 단계별 산출물이 구조화되어 단계를 끄고 켜서 그림자 비교 가능 |

**추천: D**

| 단계 | 실행 | 도구 | 산출물 |
|---|---|---|---|
| 0. 증거 묶음 생성 | 코드 | - | `evidence_pack`: 시세·재무(DART)·공시·뉴스 묶음·수급·논거 노트(주장 목록)·원칙. 모든 항목에 ID와 수집 시각 |
| 1. 검색 계획 | 코드 (또는 빠른 모델이 템플릿에서 고름) | 없음 | 검색어 목록. 종목명과 주제어만 |
| 2. 웹 수집 | AI 실행 S (빠른 모델 또는 중간 모델) | WebSearch(게이트 W2), `search_news`, `get_disclosures`. **보유 내역 도구 없음** | 검색 결과를 원장에 `web`으로 저장, 요약은 `untrusted_text`로 |
| 3a. 긍정론 | AI 실행 B (최상위 모델) | 읽기 도구(보유 내역 포함), **WebSearch 없음** | `case` 스키마: 주장마다 `evidence_refs` |
| 3b. 부정론 | AI 실행 R (가능하면 다른 제공자나 다른 모델) | 같음. **3a 결과를 보지 않음** | 같은 스키마 + 사전 부검 형식("6개월 뒤 이 투자가 실패했다면 이유는") |
| 4. 주장 검증 | 코드 + AI 실행 V (분리 방식: 주장 하나씩, 초안을 보지 않음) | 원장만 | 주장별 `supported`, `contradicted`, `unsupported`. 코드가 숫자 `ref`를 대조 |
| 5. 결론 | AI 실행 J (3a·3b를 쓰지 않은 모델이 바람직) | 검증된 주장 목록만 | 논거 노트 차분, `idea` 또는 `propose_order` 호출. 확률 p 포함 |
| 6. 게이트 | 코드 | - | 원칙 검사, 출처 종류 경고, 개수 제한, 노트 상태 집계 |

- 토론 왕복(반박에 대한 재반박)은 넣지 않는다. 3a와 3b는 한 번씩만 쓴다.
- 기준선으로 A(단일 호출)를 그림자로 같이 돌린다. 성적표에서 D가 A보다 나은지 확인하는 장치다(2.4-7).
- 채팅 중에 심층 연구가 멈춰야 하므로(AI-1) 단계 경계마다 중간 산출물을 저장하고 재개할 수 있게 한다.

**추천이 틀렸다고 판명될 조건**

- 그림자 실행에서 확정된 제안이 충분히 쌓였을 때(3.4의 기준, 대략 200건 이상), D와 A의 짝지은 초과 수익 차이 신뢰구간이 0을 포함하거나 A가 낫다면, 비용 대비 D를 유지할 근거가 없다. 그때는 A나 B로 줄인다.
- 3b(부정론)를 다른 모델로 바꿔도 결론이 3a 쪽으로 쏠리는 비율이 같은 모델일 때와 차이가 없다면, 이질성의 이득은 이 작업에서 확인되지 않은 것이다.
- 2단계 분리 때문에 분석 실행이 필요한 정보를 못 얻어 "출처 없음"이나 "원인 불명" 비율이 크게 오른다면, W2 게이트만 두고 실행 분리를 완화하는 것을 검토한다(유출 위험이 커지는 것을 감수).

### 3.2 확신 정도 표현 비교

| 방식 | 보정 측정 | 과신 영향 | 추천 |
|---|---|---|---|
| 서술형(높음, 중간, 낮음) | 불가 | 큼 | 아니오 |
| 1~10 점수 | 사후 매핑 필요, 사건 정의 없음 | 큼 | 아니오 |
| 정의된 사건의 확률 p (5% 단위) | Brier, 신뢰도 도표 | 측정하고 보정할 수 있음 | **예** |
| 여러 번 뽑은 p의 평균 | 같음 | 분산 감소 | 토큰 여유가 있으면 예 (2402.19379) |

### 3.3 제안 스키마 초안

JSON Schema 2020-12 형식이다. 금액·가격·수량은 문자열 십진수다(ACC-2). 코드가 덮어쓰는 필드는 AI 입력 스키마에 없고 저장 레코드에만 있다.

**`propose_order` 입력 (AI가 보냄)**

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "stock-copilot/propose_order.input.v1",
  "type": "object",
  "additionalProperties": false,
  "required": ["symbol", "side", "order_type", "quantity", "horizon_days", "thesis_link", "reasons", "counter_arguments", "invalidation", "probability"],
  "properties": {
    "symbol": { "type": "string", "pattern": "^[A-Z0-9.]{1,12}$" },
    "side": { "enum": ["buy", "sell"] },
    "order_type": { "enum": ["limit", "market"] },
    "quantity": { "type": "string", "pattern": "^[0-9]+(\\.[0-9]{1,6})?$" },
    "limit_price": { "type": "string", "pattern": "^[0-9]+(\\.[0-9]{1,4})?$" },
    "target_weight_range": {
      "type": "object", "additionalProperties": false,
      "required": ["min_pct", "max_pct"],
      "properties": { "min_pct": { "type": "string" }, "max_pct": { "type": "string" } }
    },
    "horizon_days": { "enum": [5, 20, 60] },
    "thesis_link": {
      "type": "object", "additionalProperties": false,
      "required": ["thesis_id", "claim_ids"],
      "properties": {
        "thesis_id": { "type": "string" },
        "claim_ids": { "type": "array", "items": { "type": "string" }, "minItems": 1 }
      }
    },
    "reasons": {
      "type": "array", "minItems": 1, "maxItems": 5,
      "items": { "$ref": "#/$defs/claim" }
    },
    "counter_arguments": {
      "type": "array", "minItems": 1, "maxItems": 5,
      "items": { "$ref": "#/$defs/claim" }
    },
    "invalidation": {
      "type": "object", "additionalProperties": false,
      "required": ["machine", "narrative"],
      "properties": {
        "machine": {
          "type": "array",
          "items": {
            "type": "object", "additionalProperties": false,
            "required": ["kind", "op", "value"],
            "properties": {
              "kind": { "enum": ["price", "date", "excess_return_pct", "disclosure_type"] },
              "op": { "enum": ["lte", "gte", "eq"] },
              "value": { "type": "string" }
            }
          }
        },
        "narrative": { "type": "string", "maxLength": 500 }
      }
    },
    "probability": {
      "type": "object", "additionalProperties": false,
      "required": ["event", "p"],
      "properties": {
        "event": { "const": "excess_return_positive_over_horizon" },
        "p": { "type": "number", "minimum": 0.05, "maximum": 0.95, "multipleOf": 0.05 }
      }
    }
  },
  "$defs": {
    "claim": {
      "type": "object", "additionalProperties": false,
      "required": ["text", "evidence_refs"],
      "properties": {
        "text": { "type": "string", "maxLength": 400 },
        "evidence_refs": {
          "type": "array", "minItems": 1,
          "items": {
            "type": "object", "additionalProperties": false,
            "required": ["ref"],
            "properties": { "ref": { "type": "string", "pattern": "^(tool|news|disclosure|web):[A-Za-z0-9_\\-#/.]+$" } }
          }
        },
        "numbers": {
          "type": "array",
          "items": {
            "type": "object", "additionalProperties": false,
            "required": ["label", "ref"],
            "properties": { "label": { "type": "string" }, "ref": { "type": "string" } }
          }
        }
      }
    }
  }
}
```

`probability.p`의 `multipleOf: 0.05`는 부동소수점 비교 문제가 있을 수 있다. 실제로는 정수 퍼센트(`p_pct`: 5~95, 5의 배수)로 받는 편이 안전하다. ACC-2의 부동소수점 금지와도 맞다.

**저장 레코드에 코드가 붙이는 필드**

| 필드 | 내용 |
|---|---|
| `proposal_id`, `run_id`, `created_at`, `expires_at` | 6장 규칙 (10분 또는 가격 1% 변동) |
| `provider`, `model`, `effort`, `prompt_id`, `prompt_version`, `prompt_hash`, `cli_version` | AI-5, 2.8 |
| `pipeline_variant` | `D`, `A-shadow` 등. 그림자 실행은 사용자에게 보이지 않음 |
| `reference_price`, `reference_price_at`, `benchmark_symbol`, `benchmark_level` | 성적표 기준 (2.4) |
| `computed`: `order_amount`, `fee_estimate`, `weight_before`, `weight_after`, `fx_rate`, `fx_at` | 코드 계산 (ACC-1, ACC-3) |
| `checks[]` | 원칙·가능금액·유의사항 검사 결과 |
| `evidence_profile` | 근거 출처 종류의 개수. `web`·`news`뿐이면 경고 |
| `unverified_numbers[]` | 원장과 맞지 않은 숫자 (2.6) |
| `claim_verification[]` | 4단계 결과 |
| `decision`, `decision_reason`, `decided_at` | 승인, 거절, 만료, 수정 |
| `outcome[horizon]` | 기간별 종목 수익률, 벤치마크 수익률, 초과 수익, 가상 체결 여부, 확정 시각 |

**`idea`**: 위 입력에서 `quantity`, `limit_price`, `order_type`을 빼고 `watch_triggers[]`(주문 제안으로 바꿀 조건, `invalidation.machine`과 같은 형식)를 더한다.

**`thesis` 차분 (`update_thesis` 입력)**: `{thesis_id, base_version, ops[]: {op: add_claim|update_claim_status|add_kill_criterion|add_catalyst|edit_summary, target_id?, payload, evidence_refs[]}}`. `base_version`이 현재 버전과 다르면 거부한다. 출처가 `web`뿐인 연산은 "확인 필요"로 보류한다.

### 3.4 성적표 표본 크기 계산 (직접 계산)

`/private/tmp/claude-501/.../scratchpad/research/R-12/samplesize.py`로 계산했다(유의수준 5% 양측, 검정력 80%).

```
평균 초과 수익 검출: n = ((1.96+0.84)·σ/μ)²
  σ=6%  μ=1% n=283 | μ=2% n=71  | μ=3% n=32
  σ=10% μ=1% n=785 | μ=2% n=197 | μ=3% n=88
  σ=15% μ=1% n=1767| μ=2% n=442 | μ=3% n=197
적중률 p를 50%와 구분: p=0.55 n=783, p=0.60 n=194, p=0.65 n=85
관측 적중률 60%의 95% Wilson 구간:
  n=10 [0.31, 0.83]  n=20 [0.39, 0.78]  n=50 [0.46, 0.72]  n=100 [0.50, 0.69]  n=200 [0.53, 0.67]
```

σ는 가정값이다. 같은 날 제안끼리의 상관을 반영하면 필요한 건수는 더 늘어난다. 하루 1~2건의 제안이면 결론까지 1년 가까이 걸린다. 성적표의 기본 표시가 "판정 보류"여야 하는 이유다.

## 4. 위험과 미확인 항목

**위험**

- LLM 제안의 기대 성과가 0이거나 음수일 수 있다. 장기 검증은 그쪽을 가리킨다(FINSABER). 성적표가 이를 드러낼 때까지 오래 걸리므로, 그동안 사용자가 운이나 국면을 실력으로 착각할 수 있다.
- 거절 사유를 다음 제안의 맥락으로 넣는 규칙(6장)은 아첨을 키워 "사용자가 승인할 만한 제안"으로 쏠리게 할 수 있다.
- 실행 분리(W1)를 하면 채팅의 즉흥적인 웹 검색이 불편해진다. 사용자가 편의를 위해 분리를 끄고 싶어질 수 있다.
- 검색어 게이트 규칙은 우회될 수 있다. 숫자를 한글로 쓰거나("백이십 주") 종목명 대신 다른 표현을 쓰는 식이다. 게이트는 정밀 수치의 유출을 줄일 뿐 관심 종목 노출은 막지 못한다. 근본 대책은 W1이다.
- 그림자 실행은 구독 한도를 두 배로 쓴다. AI-2 예산 안에서 표본 추출(예: 제안의 30%만 그림자)로 돌려야 한다.

**미확인**

- Anthropic 웹 검색 백엔드가 질의 속 URL을 방문하는지, 검색 제공자가 질의를 얼마나 보관하는지. 하위 처리자 1차 페이지를 읽지 못했다.
- `--permission-prompt-tool`의 `updatedInput`으로 `allowed_domains`를 강제로 덮어쓸 수 있는지(실험하지 않음).
- Codex CLI의 웹 검색 질의를 앱이 실행 전에 가로채 판정할 수단이 있는지 (R-03).
- 앱의 실제 MCP 서버 전송(R-04)과 격리 옵션 조합(R-02)에서도 게이트가 같은 방식으로 동작하는지.
- TradingAgents의 독립 재현. ForecastBench 관련 수치(Platt 스케일링 효과)의 1차 원문.
- OWASP LLM Top 10 2025 공식 페이지 원문.
- 논거 노트 구조나 사전 부검이 투자 판단의 정확도를 올린다는 직접 증거.
- 이 앱 사용자의 실제 초과 수익 표준편차(σ). 데이터가 쌓인 뒤 추정한다.
- promptfoo가 CLI 자식 프로세스 제공자를 지원하는지.

## 5. 이 결과로 바뀌어야 할 설계 문서 항목

| 문서·위치 | 현재 | 바꿀 내용 |
|---|---|---|
| design.md 2장 공통 원칙 4번째 | "AI는 임의의 웹 주소를 직접 읽지 못한다…" | 추가: "웹 검색어는 외부로 나가는 통로다. 보유 내역 도구와 웹 검색은 한 실행에 함께 주지 않는다. 함께 줄 수밖에 없는 채팅에서는 코드가 검색어를 실행 전에 판정한다" |
| design.md 3장 MCP 도구 목록 | 제안 도구 필드가 서술형 | `propose_order`·`propose_idea`·`update_thesis`의 입력 스키마를 3.3으로 고정. 권한 판정 전용 도구(`gate_tool`, 모델에게 숨김)를 추가. `get_portfolio`에 요약 모드(비중 구간) 추가 |
| design.md 4.1 Claude Code 실행 방식 | `--tools WebSearch` | `--permission-prompt-tool mcp__<app>__gate_tool`을 추가하고 WebSearch를 허용 목록에 넣지 않는다. 게이트 도구는 `--disallowedTools`로 모델에게서 숨긴다. 단계별로 도구 집합이 다른 실행 프로필(수집, 분석, 채팅)을 정의한다 |
| design.md 4.3 심층 연구 | 자료 수집 → 긍정론 → 부정론 → 반박 검증 → 결론 | 3.1의 D로 교체: 증거 묶음(코드) → 웹 수집(보유 내역 없음) → 긍정론·부정론(서로 보지 않음, 다른 모델 권장) → 분리 주장 검증(원장 대조) → 결론 → 코드 게이트. 토론 왕복 없음. 기준선 A를 그림자 실행 |
| design.md 4.3 표 | 아침 브리핑·마감 리뷰·발굴의 내용만 있음 | 2.9의 입력, 단계, 산출물 스키마를 덧붙임. 발굴 후보는 코드가 뽑고 시가총액 구간별 할당 |
| design.md 2장 D4 성적표 | "이후 수익률을 기록해 벤치마크와 비교" | 2.4의 방법론: 제안 시각 기준 가격, 고정 기간 5·20·60거래일, 시장별 벤치마크, 가상 체결 규칙, 30건 미만 판정 보류, 블록 부트스트랩 신뢰구간, 비교한 조합 수 표시 |
| design.md 2장 D6 논거 노트 | "왜 들고 있는지, 언제 팔 것인지" 문서 | 2.2의 구조: 주장 목록, 종료 기준(코드형과 서술형), 일정, 주장별 상태, 상태 집계는 코드 |
| design.md 6장 승인 카드 | 사실과 AI 근거 영역 | 근거별 출처 종류 표시, "외부 출처만" 경고, "검증 안 됨" 숫자 표시, 확률 p와 해당 구간의 과거 적중률 |
| design.md 6장 규칙 "거절 사유를 다음 제안의 맥락으로" | 그대로 맥락 투입 | 별도 필드로 주고 "사용자 의견은 근거가 아님"을 명시. 효과는 성적표로 감시 |
| design.md SEC-2 | "앱 MCP와 웹 검색만으로 실행" | "실행 프로필별 도구 집합. 보유 내역 도구가 있는 프로필에서 웹 검색은 검색어 게이트를 통과해야 한다" |
| design.md 7.1 (새 항목 SEC-9) | 없음 | "모든 웹 검색어는 실행 전에 코드가 판정하고 감사 로그에 남긴다. 수량·금액·계좌 정보가 든 검색어는 0건이어야 한다. 테스트로 고정한다" |
| design.md ACC-1 | "MCP 도구 결과와 대조" | 구현 방식 명시: 구조화 출력의 숫자는 원장 `ref`, 자유 서술의 숫자는 정규식 추출 후 대조, 웹 값은 "미검증 웹 값" |
| design.md AI-5 | 모델별 집계 | 그림자 실행으로 같은 입력에 대한 짝지은 비교. 예산 안에서 표본 추출 |
| design.md 4.1 프롬프트 관리 | "앱 저장소의 파일로 관리" | 2.8의 앞머리 형식, 공용 출력 스키마(Claude `--json-schema`, Codex `--output-schema`), 고정 재생 세트로 버전 비교 |
| architecture.md 신뢰 경계 표 (AI CLI 행) | "웹 검색·읽기" | "웹 검색(게이트 통과)". 웹 읽기 없음. design.md와 일치시킬 것 (이 조사에서는 수정하지 않음) |
| architecture.md 저장소 "AI 작업" | 프롬프트 버전, 모델, 사용량, 결과 | 프롬프트 해시, CLI 버전, 추론 강도, 파이프라인 변형, 증거 묶음 ID, 도구 결과 원장 추가 |
| research.md R-02 | 격리 옵션 질문 | 추가 확인: `--permission-prompt-tool`이 실제 앱 격리 옵션 조합과 함께 동작하는지, `updatedInput`으로 `allowed_domains`를 강제할 수 있는지 |
| research.md R-03 | Codex 격리 | 추가 확인: Codex 웹 검색의 질의를 실행 전에 가로챌 수단. 없으면 Codex는 보유 내역 프로필에서 웹 검색을 끈다 |

## 6. 출처 목록 (모두 2026-11-11 확인)

**LLM 투자 분석과 실패 유형**
- Xiao et al., TradingAgents: Multi-Agents LLM Financial Trading Framework, arXiv 2412.20138 v4. https://arxiv.org/html/2412.20138v4
- Hugging Face papers 2412.20138 (댓글). https://huggingface.co/papers/2412.20138
- Li et al., Can LLM-based Financial Investing Strategies Outperform the Market in Long Run? (FINSABER), arXiv 2505.07078 v5. https://arxiv.org/html/2505.07078v5
- Lopez-Lira & Tang, Can ChatGPT Forecast Stock Price Movements?, arXiv 2304.07619. https://arxiv.org/abs/2304.07619v5
- Lopez-Lira et al., The Memorization Problem, arXiv 2504.14765. https://arxiv.org/html/2504.14765v2
- Gao, Jiang, Yan, A Test of Lookahead Bias in LLM Forecasts, arXiv 2512.23847. https://arxiv.org/html/2512.23847v1
- Islam et al., FinanceBench, arXiv 2311.11944. https://web3.arxiv.org/abs/2311.11944
- Patronus AI 발표 (2023-11). https://prnewswire.com/news-releases/patronus-ai-launches-industry-first-llm-benchmark-for-finance-to-address-hallucinations-301990858.html
- Your AI, Not Your View: The Bias of LLMs in Investment Analysis, arXiv 2507.20957. https://arxiv.org/pdf/2507.20957
- Exposing Product Bias in LLM Investment Recommendation, arXiv 2503.08750. https://arxiv.org/pdf/2503.08750
- LLM 실험 자산 시장 연구, arXiv 2502.15800. https://arxiv.org/pdf/2502.15800v3
- Sharma et al., Towards Understanding Sycophancy in Language Models, arXiv 2310.13548. https://arxiv.org/abs/2310.13548
- Panickssery, Bowman, Feng, LLM Evaluators Recognize and Favor Their Own Generations, NeurIPS 2024. https://papers.nips.cc/paper_files/paper/2024/hash/7f1f0218e45f5414c79c0679633e47bc-Abstract-Conference.html

**토론, 집계, 검증, 토큰**
- Huang et al., Large Language Models Cannot Self-Correct Reasoning Yet, arXiv 2310.01798. https://arxiv.org/pdf/2310.01798
- Smit et al., Should we be going MAD?, arXiv 2311.17371. https://arxiv.org/html/2311.17371v3
- Zhang et al., Stop Overvaluing Multi-Agent Debate, arXiv 2502.08788. https://arxiv.org/html/2502.08788v3
- Li et al., More Agents Is All You Need, arXiv 2402.05120. https://arxiv.org/abs/2402.05120
- Dhuliawala et al., Chain-of-Verification Reduces Hallucination, arXiv 2309.11495. https://arxiv.org/abs/2309.11495
- Anthropic Engineering, How we built our multi-agent research system (2025-06-13). https://www.anthropic.com/engineering/multi-agent-research-system

**보정과 예측**
- Xiong et al., Can LLMs Express Their Uncertainty?, arXiv 2306.13063. https://arxiv.org/pdf/2306.13063
- 추론 모델 보정, arXiv 2505.14489 v2. https://arxiv.org/pdf/2505.14489v2
- Schoenegger et al., Wisdom of the Silicon Crowd, arXiv 2402.19379 (Science Advances). https://arxiv.org/html/2402.19379v6
- AI forecasting in 2026: what 11 analyses say (2차 정리). https://www.lesswrong.com/posts/a82q6yd8zKpYk56cF/ai-forecasting-in-2026-what-11-analyses-say

**성적표 방법론**
- Bailey & López de Prado, The Deflated Sharpe Ratio, SSRN 2460551. https://papers.ssrn.com/abstract=2460551
- López de Prado, Sharpe ratio 강의 자료 (MinTRL, MinBTL). https://newyork.qwafafew.org/wp-content/uploads/sites/4/2017/01/Lopez_de_Prado_Sharpe.pdf
- Barber, Lehavy, McNichols, Trueman, Can Investors Profit from the Prophets?, Journal of Finance 56(2), 2001. https://ideas.repec.org/a/bla/jfinan/v56y2001i2p531-563.html
- Klein, Performing a Project Premortem (HBR 2007) 수록본. https://www.oreilly.com/library/view/hbr-guide-to/9781422187319/14-chap5.xhtml
- 사전 부검 30% 수치의 해석. https://resources.rework.com/libraries/project-management/pre-mortem

**프롬프트 주입과 유출**
- Willison, The lethal trifecta for AI agents (2025-06-16). https://simonwillison.net/2025/Jun/16/the-lethal-trifecta/
- Willison, Notion 3.0 lethal trifecta (2025-09-19). https://simonwillison.net/2025/Sep/19/notion-lethal-trifecta
- Rall et al., Exploiting Web Search Tools of AI Agents for Data Exfiltration, arXiv 2510.09093. https://arxiv.org/html/2510.09093v1
- Beurer-Kellner et al., Design Patterns for Securing LLM Agents against Prompt Injections, arXiv 2506.08837. https://arxiv.org/html/2506.08837v3
- Debenedetti et al., Defeating Prompt Injections by Design (CaMeL), arXiv 2503.18813. https://arxiv.org/pdf/2503.18813
- Hines et al., Defending Against Indirect Prompt Injection Attacks With Spotlighting, arXiv 2403.14720. https://arxiv.org/pdf/2403.14720
- OWASP LLM Top 10 2025 정리 (2차). https://opensourcesecurity.substack.com/p/a-deep-dive-into-the-owasp-top-10

**Claude Code와 웹 검색 (공식 문서)**
- Claude Code tools reference, WebSearch tool behavior. https://code.claude.com/docs/en/tools-reference
- Claude Code permissions. https://code.claude.com/docs/en/permissions
- Claude Code hooks reference (PreToolUse, WebSearch 입력, updatedInput). https://code.claude.com/docs/en/hooks
- Claude Code CLI reference (`--permission-prompt-tool`, `--json-schema`, `--tools`, `--disallowedTools`, `--setting-sources`). https://code.claude.com/docs/en/cli-reference
- Claude Code errors (권한 프롬프트 도구 반환 형식). https://code.claude.com/docs/en/errors
- Claude API web search tool (`web_search_20250305`, `web_search_20260209`, `web_search_20260318`, 도메인 필터, max_uses). https://platform.claude.com/docs/en/agents-and-tools/tool-use/web-search-tool
- Anthropic 하위 처리자 페이지 (내용 읽기 실패). https://trust.anthropic.com/subprocessors
- TechCrunch, Anthropic appears to be using Brave (2025-03-21). https://techcrunch.com/2025/03/21/anthropic-appears-to-be-using-brave-to-power-web-searches-for-its-claude-chatbot/
- Willison, Anthropic use Brave (2025-03-21). https://simonwillison.net/2025/Mar/21/anthropic-use-brave/

**도구**
- promptfoo getting started. https://promptfoo.dev/docs/getting-started/
- Codex CLI 0.167.1 `codex exec --help` (로컬 실행 결과, `--output-schema`)
