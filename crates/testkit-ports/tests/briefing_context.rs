#[cfg(test)]
mod tests {
    use application::{
        BriefingContextSource, HoldingsBriefingContext, NewsRepository, PortfolioRepository,
        SystemFailure, SystemPort, SystemPortError, WriteCompleted,
    };
    use domain::{
        AccountId, BriefingKind, BuyingPower, Currency, InformationSource, InstrumentId, Money,
        Nanosecond, NewsItem, NewsSource, NormalizedUrl, NumericInput, OffsetSeconds, Portfolio,
        Symbol, SyncOutcome, SyncRecord, Timestamp, UnixSeconds, UntrustedText,
    };
    use std::collections::BTreeMap;
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("즉시 완료"),
        }
    }

    fn at() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(1_795_995_600),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }

    fn money(value: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(value).expect("입력"), currency).expect("금액")
    }

    fn holding(symbol: &str) -> domain::Holding {
        let master = domain::MasterInstrument::try_new(domain::MasterInstrumentRow {
            listing_market: domain::MasterCode::try_new("NASDAQ").expect("시장"),
            instrument_id: InstrumentId::try_new(symbol).expect("ID"),
            symbol: Symbol::try_new(symbol).expect("심볼"),
            name: domain::DisplayInstrumentName::try_new("보유 종목").expect("이름"),
            security_type: domain::MasterSecurityType::from_code(
                domain::MasterCode::try_new("STOCK").expect("종류"),
            ),
            currency: Currency::Usd,
            listing_status: domain::ListingStatus::Active,
            common_share: domain::CommonShare::Common,
            korean_detail: None,
            observed_at: at(),
        })
        .expect("마스터");
        domain::Holding::new(
            master.order_instrument().expect("종목"),
            domain::Quantity::parse(&NumericInput::try_new("3").expect("수량 입력")).expect("수량"),
            domain::AcquisitionPrice::try_new(money("11", Currency::Usd)).expect("원가"),
            domain::HoldingObservation::new(None, None, at()),
        )
        .expect("보유")
    }

    fn portfolio(symbols: &[&str]) -> Portfolio {
        Portfolio::new(
            symbols.iter().map(|symbol| holding(symbol)).collect(),
            BuyingPower::new(money("1", Currency::Krw), at()),
            BuyingPower::new(money("1", Currency::Usd), at()),
        )
        .expect("포트폴리오")
    }

    #[derive(Default)]
    struct Portfolios(BTreeMap<AccountId, SyncOutcome<Portfolio>>);

    impl PortfolioRepository for Portfolios {
        fn record(
            &self,
            _account: &AccountId,
            _record: &SyncRecord<Portfolio>,
        ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
            std::future::ready(Ok(WriteCompleted))
        }
        fn latest(
            &self,
            account: &AccountId,
        ) -> impl Future<Output = Result<Option<SyncRecord<Portfolio>>, SystemPortError>> + Send
        {
            std::future::ready(Ok(self
                .0
                .get(account)
                .map(|outcome| SyncRecord::new(at(), outcome.clone()))))
        }
    }

    struct News(Option<SystemPortError>);

    fn article(path: &str) -> NewsItem {
        NewsItem::new(
            NewsSource::Yonhap,
            NormalizedUrl::try_new(&format!("https://example.com/{path}")).expect("주소"),
            UntrustedText::try_new(path).expect("제목"),
            None,
        )
    }

    impl NewsRepository for News {
        fn store(
            &self,
            _item: &NewsItem,
        ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
            std::future::ready(Ok(WriteCompleted))
        }
        fn for_instrument(
            &self,
            instrument: &InstrumentId,
        ) -> impl Future<Output = Result<Vec<NewsItem>, SystemPortError>> + Send {
            let items = (0_u32..7_u32)
                .map(|index| article(&format!("{}-{index}", instrument.as_str())))
                .collect();
            std::future::ready(self.0.map_or(Ok(items), Err))
        }
    }

    struct Filings(domain::Disclosure);

    impl application::DisclosureRepository for Filings {
        async fn store_disclosure(
            &self,
            _: &domain::Disclosure,
        ) -> Result<WriteCompleted, SystemPortError> {
            Ok(WriteCompleted)
        }
        async fn disclosure(
            &self,
            _: &domain::DisclosureKey,
        ) -> Result<Option<domain::Disclosure>, SystemPortError> {
            Ok(Some(self.0.clone()))
        }
        async fn disclosures_for_instrument(
            &self,
            _: &InstrumentId,
        ) -> Result<Vec<domain::Disclosure>, SystemPortError> {
            Ok(vec![self.0.clone()])
        }
    }

    #[test]
    fn scheduled_context_appends_one_source_for_a_filing_linked_to_two_holdings() {
        let mut portfolios = Portfolios::default();
        let _saved = portfolios.0.insert(
            AccountId::new(1),
            SyncOutcome::Complete(portfolio(&["AAPL", "MSFT"])),
        );
        let receipt = domain::DartReceiptNo::try_new("20261113000001").expect("접수번호");
        let url =
            NormalizedUrl::try_new("https://dart.fss.or.kr/dsaf001/main.do?rcpNo=20261113000001")
                .expect("원문");
        let title = UntrustedText::try_new("실적 공시").expect("제목");
        let filing = domain::Disclosure::new(
            domain::ReceiptNo::Dart(receipt),
            url.clone(),
            title.clone(),
            None,
        );
        let context = run(HoldingsBriefingContext {
            portfolios: &portfolios,
            news: &News(None),
            accounts: &[AccountId::new(1)],
        }
        .with_disclosures(&Filings(filing))
        .context(BriefingKind::Morning, at()))
        .expect("문맥");
        assert_eq!(context.evidence.len(), 11);
        let source = context.evidence.last().expect("공시");
        assert_eq!(
            source.origin,
            InformationSource::Disclosure(domain::DisclosureSource::Dart)
        );
        assert_eq!(source.url.as_ref(), Some(&url));
        assert_eq!(source.title, title);
    }

    #[test]
    fn context_lists_unique_held_symbols_across_accounts_with_capped_news_sources() {
        let mut portfolios = Portfolios::default();
        let _first = portfolios.0.insert(
            AccountId::new(1),
            SyncOutcome::Complete(portfolio(&["AAPL", "MSFT"])),
        );
        let _second = portfolios.0.insert(
            AccountId::new(2),
            SyncOutcome::Partial(portfolio(&["AAPL"])),
        );
        let _third = portfolios.0.insert(AccountId::new(3), SyncOutcome::Failed);
        let accounts = [
            AccountId::new(1),
            AccountId::new(2),
            AccountId::new(3),
            AccountId::new(4),
        ];
        let context = run(HoldingsBriefingContext {
            portfolios: &portfolios,
            news: &News(None),
            accounts: &accounts,
        }
        .context(BriefingKind::Morning, at()))
        .expect("문맥");
        assert_eq!(
            context.instruments,
            vec![
                Symbol::try_new("AAPL").expect("심볼"),
                Symbol::try_new("MSFT").expect("심볼")
            ]
        );
        assert_eq!(context.evidence.len(), 10);
        assert_eq!(
            context.evidence[0].origin,
            InformationSource::News(NewsSource::Yonhap)
        );
        assert_eq!(
            context.evidence[0].title,
            UntrustedText::try_new("AAPL-0").expect("제목")
        );
        assert_eq!(context.code_facts, Vec::new());
    }

    #[test]
    fn context_without_holdings_is_empty_and_news_failures_are_reported() {
        let empty = run(HoldingsBriefingContext {
            portfolios: &Portfolios::default(),
            news: &News(None),
            accounts: &[AccountId::new(1)],
        }
        .context(BriefingKind::ClosingReview, at()))
        .expect("문맥");
        assert_eq!(empty.instruments, Vec::new());
        assert_eq!(empty.evidence, Vec::new());
        let mut portfolios = Portfolios::default();
        let _held = portfolios.0.insert(
            AccountId::new(1),
            SyncOutcome::Complete(portfolio(&["AAPL"])),
        );
        let failure = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        assert_eq!(
            run(HoldingsBriefingContext {
                portfolios: &portfolios,
                news: &News(Some(failure)),
                accounts: &[AccountId::new(1)],
            }
            .context(BriefingKind::Morning, at())),
            Err(failure)
        );
    }
}
