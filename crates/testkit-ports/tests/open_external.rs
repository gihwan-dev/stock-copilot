#[cfg(test)]
mod tests {
    use application::{
        CollectedUrlCatalog, ExternalUrlOpened, ExternalUrlOpener, ExternalUrlOpenerError,
        OpenExternalError, OpenExternalUrl, RetryDelay, SystemFailure, SystemPort, SystemPortError,
    };
    use domain::{
        ExternalUrlAllowance, ExternalUrlInput, ExternalUrlRejection, FixedExternalUrl,
        NormalizedUrl, ParsedExternalUrl,
    };
    use std::future::Future;
    use std::task::{Context, Poll, Waker};
    use testkit_ports::memory::MemoryReply;
    use testkit_ports::system_fakes::{
        CollectedUrlCatalogCall, ExternalUrlOpenerCall, MemoryCollectedUrlCatalog,
        ScriptedExternalUrlOpener,
    };

    fn block_on<F: Future>(future: F) -> F::Output {
        let mut pinned = Box::pin(future);
        let mut context = Context::from_waker(Waker::noop());
        loop {
            match pinned.as_mut().poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => {}
            }
        }
    }

    #[test]
    fn fake_catalog_and_opener_allow_fixed_and_collected_https_only() {
        let catalog = MemoryCollectedUrlCatalog::default();
        let opener = ScriptedExternalUrlOpener::default();
        let use_case = OpenExternalUrl::new(&catalog, &opener);

        let fixed_url = "https://claude.ai/settings/data-privacy-controls";
        opener.script_open().push(Ok(ExternalUrlOpened));
        let fixed_outcome =
            block_on(use_case.execute(&ExternalUrlInput::new(fixed_url))).expect("고정 주소 열기");
        assert_eq!(
            fixed_outcome.allowance(),
            ExternalUrlAllowance::Fixed(FixedExternalUrl::AnthropicDataPrivacyControls)
        );
        assert_eq!(catalog.calls(), Vec::new());
        assert_eq!(opener.opened_urls(), vec![String::from(fixed_url)]);

        let news_url = "https://www.mk.co.kr/news/stock/12205485";
        catalog.insert(NormalizedUrl::try_new(news_url).expect("뉴스 주소"));
        catalog.insert(
            NormalizedUrl::try_new("http://www.edaily.co.kr/news/1").expect("http 뉴스 주소"),
        );

        catalog.script_find_collected().push(MemoryReply::Memory);
        opener.script_open().push(Ok(ExternalUrlOpened));
        let news_outcome =
            block_on(use_case.execute(&ExternalUrlInput::new(news_url))).expect("수집 뉴스 열기");
        assert_eq!(news_outcome.allowance(), ExternalUrlAllowance::Collected);
        assert_eq!(
            catalog.calls(),
            vec![CollectedUrlCatalogCall::FindCollected(
                ParsedExternalUrl::parse(&ExternalUrlInput::new(news_url)).expect("파싱")
            )]
        );
        assert_eq!(
            opener.opened_urls(),
            vec![String::from(fixed_url), String::from(news_url)]
        );
    }

    #[test]
    fn fake_catalog_rejects_http_upgrade_and_prefix_bypasses() {
        let catalog = MemoryCollectedUrlCatalog::default();
        let opener = ScriptedExternalUrlOpener::default();
        let use_case = OpenExternalUrl::new(&catalog, &opener);

        catalog.insert(
            NormalizedUrl::try_new("https://www.mk.co.kr/news/stock/12205485").expect("뉴스"),
        );
        catalog
            .insert(NormalizedUrl::try_new("http://www.edaily.co.kr/news/1").expect("http 뉴스"));

        let http_err =
            block_on(use_case.execute(&ExternalUrlInput::new("http://www.edaily.co.kr/news/1")))
                .expect_err("http 거부");
        assert_eq!(
            http_err,
            OpenExternalError::Rejected(ExternalUrlRejection::NonHttpsScheme)
        );

        catalog.script_find_collected().push(MemoryReply::Memory);
        let upgraded_err =
            block_on(use_case.execute(&ExternalUrlInput::new("https://www.edaily.co.kr/news/1")))
                .expect_err("http 항목을 https로 자동 승격 금지");
        assert_eq!(upgraded_err, OpenExternalError::NotAllowed);

        catalog.script_find_collected().push(MemoryReply::Memory);
        let prefix_err = block_on(use_case.execute(&ExternalUrlInput::new(
            "https://www.mk.co.kr/news/stock/12205485/redirect",
        )))
        .expect_err("접두 우회 거부");
        assert_eq!(prefix_err, OpenExternalError::NotAllowed);
        assert_eq!(opener.calls(), Vec::<ExternalUrlOpenerCall>::new());
    }

    #[test]
    fn fake_catalog_failure_and_mismatched_reply_never_invoke_opener() {
        let catalog = MemoryCollectedUrlCatalog::default();
        let opener = ScriptedExternalUrlOpener::default();
        let use_case = OpenExternalUrl::new(&catalog, &opener);

        let system_error = SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        );
        catalog
            .script_find_collected()
            .push(MemoryReply::Return(Err(system_error)));
        let err = block_on(use_case.execute(&ExternalUrlInput::new(
            "https://www.mk.co.kr/news/stock/12205485",
        )))
        .expect_err("저장소 불가");
        assert_eq!(err, OpenExternalError::Catalog(system_error));
        assert_eq!(opener.calls(), Vec::<ExternalUrlOpenerCall>::new());

        catalog
            .script_find_collected()
            .push(MemoryReply::Return(Ok(Some(
                NormalizedUrl::try_new("https://www.mk.co.kr/news/stock/99999999")
                    .expect("다른 기사"),
            ))));
        let mismatch = block_on(use_case.execute(&ExternalUrlInput::new(
            "https://www.mk.co.kr/news/stock/12205485",
        )))
        .expect_err("불일치 수집 결과 거부");
        assert_eq!(mismatch, OpenExternalError::CatalogMismatch);
        assert_eq!(opener.calls(), Vec::<ExternalUrlOpenerCall>::new());

        opener
            .script_open()
            .push(Err(ExternalUrlOpenerError::PermissionDenied));
        let opener_err = block_on(use_case.execute(&ExternalUrlInput::new(
            "https://claude.ai/settings/data-privacy-controls",
        )))
        .expect_err("열기 권한 거부");
        assert_eq!(
            opener_err,
            OpenExternalError::Opener(ExternalUrlOpenerError::PermissionDenied)
        );
        assert_eq!(opener.calls().len(), 1);
    }

    #[test]
    fn fake_ports_satisfy_trait_contracts_directly() {
        let catalog = MemoryCollectedUrlCatalog::default();
        let opener = ScriptedExternalUrlOpener::default();
        let parsed = ParsedExternalUrl::parse(&ExternalUrlInput::new("https://example.com/a"))
            .expect("파싱");
        catalog.script_find_collected().push(MemoryReply::Memory);
        let found = block_on(catalog.find_collected(&parsed)).expect("조회");
        assert_eq!(found, None);

        let fixed = domain::AuthorizedExternalUrl::authorize_fixed(
            FixedExternalUrl::AnthropicDataPrivacyControls.parsed_url(),
        )
        .expect("고정 인가");
        opener.script_open().push(Ok(ExternalUrlOpened));
        let receipt = block_on(opener.open(&fixed)).expect("열기");
        assert_eq!(receipt, ExternalUrlOpened);
    }
}
