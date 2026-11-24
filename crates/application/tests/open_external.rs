#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Context, Poll, Waker};

    use application::{
        BugDiagnostic, ClassifiedError, CollectedUrlCatalog, ErrorClass, ExternalUrlOpened,
        ExternalUrlOpener, ExternalUrlOpenerError, OpenExternalError, OpenExternalUrl,
        RequestRejection, RetryDelay, SystemDiagnostic, SystemFailure, SystemPort, SystemPortError,
        UserAction,
    };
    use domain::{
        AuthorizedExternalUrl, ExternalUrlAllowance, ExternalUrlInput, ExternalUrlRejection,
        FixedExternalUrl, Nanosecond, NormalizedUrl, ParsedExternalUrl, SpanSeconds, TimeSpan,
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

    struct StubCatalog {
        calls: AtomicUsize,
        reply: Result<Option<NormalizedUrl>, SystemPortError>,
    }

    impl StubCatalog {
        const fn new(reply: Result<Option<NormalizedUrl>, SystemPortError>) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                reply,
            }
        }

        fn call_count(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl CollectedUrlCatalog for StubCatalog {
        fn find_collected(
            &self,
            _url: &ParsedExternalUrl,
        ) -> impl Future<Output = Result<Option<NormalizedUrl>, SystemPortError>> + Send {
            self.calls.fetch_add(1, Ordering::SeqCst);
            std::future::ready(self.reply.clone())
        }
    }

    struct StubOpener {
        calls: AtomicUsize,
        reply: Result<ExternalUrlOpened, ExternalUrlOpenerError>,
    }

    impl StubOpener {
        const fn new(reply: Result<ExternalUrlOpened, ExternalUrlOpenerError>) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                reply,
            }
        }

        fn call_count(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl ExternalUrlOpener for StubOpener {
        fn open(
            &self,
            _url: &AuthorizedExternalUrl,
        ) -> impl Future<Output = Result<ExternalUrlOpened, ExternalUrlOpenerError>> + Send
        {
            self.calls.fetch_add(1, Ordering::SeqCst);
            std::future::ready(self.reply)
        }
    }

    #[test]
    fn fixed_url_opens_without_querying_collected_catalog() {
        let catalog = StubCatalog::new(Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )));
        let opener = StubOpener::new(Ok(ExternalUrlOpened));
        let use_case = OpenExternalUrl::new(&catalog, &opener);
        let input = ExternalUrlInput::new("https://claude.ai/settings/data-privacy-controls");

        let outcome = block_on(use_case.execute(&input)).expect("고정 주소 열기");
        assert_eq!(
            outcome.allowance(),
            ExternalUrlAllowance::Fixed(FixedExternalUrl::AnthropicDataPrivacyControls)
        );
        assert_eq!(outcome.opened(), ExternalUrlOpened);
        assert_eq!(catalog.call_count(), 0);
        assert_eq!(opener.call_count(), 1);
    }

    #[test]
    fn collected_https_url_opens_after_exact_reverification() {
        let raw = "https://www.mk.co.kr/news/stock/12205485";
        let catalog = StubCatalog::new(Ok(Some(NormalizedUrl::try_new(raw).expect("수집 주소"))));
        let opener = StubOpener::new(Ok(ExternalUrlOpened));
        let use_case = OpenExternalUrl::new(&catalog, &opener);

        let outcome =
            block_on(use_case.execute(&ExternalUrlInput::new(raw))).expect("수집 주소 열기");
        assert_eq!(outcome.allowance(), ExternalUrlAllowance::Collected);
        assert_eq!(outcome.opened(), ExternalUrlOpened);
        assert_eq!(catalog.call_count(), 1);
        assert_eq!(opener.call_count(), 1);
    }

    #[test]
    fn invalid_or_uncollected_or_mismatched_urls_fail_closed_before_opening() {
        let catalog = StubCatalog::new(Ok(None));
        let opener = StubOpener::new(Ok(ExternalUrlOpened));
        let use_case = OpenExternalUrl::new(&catalog, &opener);

        let rejected = block_on(use_case.execute(&ExternalUrlInput::new("http://example.com/a")));
        assert_eq!(
            rejected,
            Err(OpenExternalError::Rejected(
                ExternalUrlRejection::NonHttpsScheme
            ))
        );
        assert_eq!(catalog.call_count(), 0);
        assert_eq!(opener.call_count(), 0);

        let uncollected =
            block_on(use_case.execute(&ExternalUrlInput::new("https://example.com/a")));
        assert_eq!(uncollected, Err(OpenExternalError::NotAllowed));
        assert_eq!(catalog.call_count(), 1);
        assert_eq!(opener.call_count(), 0);

        let mismatched_catalog = StubCatalog::new(Ok(Some(
            NormalizedUrl::try_new("https://example.com/other").expect("다른 수집 주소"),
        )));
        let mismatch_use_case = OpenExternalUrl::new(&mismatched_catalog, &opener);
        let mismatched =
            block_on(mismatch_use_case.execute(&ExternalUrlInput::new("https://example.com/a")));
        assert_eq!(mismatched, Err(OpenExternalError::CatalogMismatch));
        assert_eq!(opener.call_count(), 0);

        let http_catalog = StubCatalog::new(Ok(Some(
            NormalizedUrl::try_new("http://example.com/a").expect("http 수집 주소"),
        )));
        let http_use_case = OpenExternalUrl::new(&http_catalog, &opener);
        let http_mismatch =
            block_on(http_use_case.execute(&ExternalUrlInput::new("https://example.com/a")));
        assert_eq!(http_mismatch, Err(OpenExternalError::CatalogMismatch));
        assert_eq!(opener.call_count(), 0);
    }

    #[test]
    fn catalog_and_opener_errors_propagate_and_classify_cleanly() {
        let wait = RetryDelay::Known(
            TimeSpan::try_new(SpanSeconds::new(5), Nanosecond::new(0)).expect("대기"),
        );
        let catalog_error =
            SystemPortError::new(SystemPort::Repository, SystemFailure::Unavailable(wait));
        let failing_catalog = StubCatalog::new(Err(catalog_error));
        let opener = StubOpener::new(Ok(ExternalUrlOpened));
        let use_case = OpenExternalUrl::new(&failing_catalog, &opener);
        let err = block_on(use_case.execute(&ExternalUrlInput::new("https://example.com/a")))
            .expect_err("카탈로그 오류");
        assert_eq!(err, OpenExternalError::Catalog(catalog_error));
        assert_eq!(err.class(), ErrorClass::Wait(wait));
        assert_eq!(opener.call_count(), 0);

        for (opener_error, expected_class) in [
            (
                ExternalUrlOpenerError::PermissionDenied,
                ErrorClass::UserAction(UserAction::CheckPermissions),
            ),
            (
                ExternalUrlOpenerError::Unavailable(wait),
                ErrorClass::Wait(wait),
            ),
            (
                ExternalUrlOpenerError::Unavailable(RetryDelay::Unknown),
                ErrorClass::Wait(RetryDelay::Unknown),
            ),
            (
                ExternalUrlOpenerError::LaunchFailed,
                ErrorClass::Wait(RetryDelay::Unknown),
            ),
        ] {
            assert_eq!(opener_error.class(), expected_class);
            let ok_catalog = StubCatalog::new(Ok(None));
            let bad_opener = StubOpener::new(Err(opener_error));
            let fixed_use_case = OpenExternalUrl::new(&ok_catalog, &bad_opener);
            let actual = block_on(fixed_use_case.execute(&ExternalUrlInput::new(
                "https://claude.ai/settings/data-privacy-controls",
            )))
            .expect_err("열기 실패");
            assert_eq!(actual, OpenExternalError::Opener(opener_error));
            assert_eq!(actual.class(), expected_class);
        }
    }

    #[test]
    fn rejection_and_integrity_errors_classify_as_bugs() {
        assert_eq!(
            OpenExternalError::Rejected(ExternalUrlRejection::Empty).class(),
            ErrorClass::Bug(BugDiagnostic::Request(RequestRejection::MalformedRequest))
        );
        assert_eq!(
            OpenExternalError::NotAllowed.class(),
            ErrorClass::Bug(BugDiagnostic::Request(RequestRejection::MalformedRequest))
        );
        assert_eq!(
            OpenExternalError::CatalogMismatch.class(),
            ErrorClass::Bug(BugDiagnostic::System(SystemDiagnostic::RepositoryIntegrity))
        );
    }
}
