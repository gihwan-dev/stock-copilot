#[cfg(test)]
mod tests {
    use application::{InformationKey, InformationLinks, NewsRepository};
    use domain::{InstrumentId, NewsItem, NewsSource, NormalizedUrl, UntrustedText};
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::memory::{MemoryNewsRepository, MemoryReply};
    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("expected ready"),
        }
    }
    #[test]
    fn links_are_identity_based_and_updates_replace_duplicate_news() {
        let repository = MemoryNewsRepository::default();
        let url = NormalizedUrl::try_new("https://example.com/a").expect("url");
        let first = NewsItem::new(
            NewsSource::Yonhap,
            url.clone(),
            UntrustedText::try_new("첫 제목").expect("text"),
            None,
        );
        let updated = NewsItem::new(
            NewsSource::Yonhap,
            url.clone(),
            UntrustedText::try_new("새 제목").expect("text"),
            None,
        );
        let key = InformationKey::News(NewsSource::Yonhap, url);
        let first_id = InstrumentId::try_new("first").expect("id");
        let second_id = InstrumentId::try_new("second").expect("id");
        run(repository.set_instruments(&key, &[first_id.clone(), first_id.clone()])).expect("link");
        repository.script_store().push(MemoryReply::Memory);
        repository.script_store().push(MemoryReply::Memory);
        run(repository.store(&first)).expect("store");
        run(repository.store(&updated)).expect("update");
        repository.script_for_instrument().push(MemoryReply::Memory);
        assert_eq!(
            run(repository.for_instrument(&first_id)),
            Ok(vec![updated.clone()])
        );
        run(repository.set_instruments(&key, std::slice::from_ref(&second_id))).expect("replace");
        repository.script_for_instrument().push(MemoryReply::Memory);
        repository.script_for_instrument().push(MemoryReply::Memory);
        assert_eq!(run(repository.for_instrument(&first_id)), Ok(vec![]));
        assert_eq!(
            run(repository.for_instrument(&second_id)),
            Ok(vec![updated])
        );
    }
}
