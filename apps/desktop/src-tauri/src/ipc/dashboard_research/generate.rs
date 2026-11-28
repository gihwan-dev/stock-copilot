use super::{
    AppHandle, DashboardCacheKey, PortTypes, Ports, ResearchEntry, ResearchSection, ResearchSource,
    ResearchStatus, Runtime, inputs, kst_iso,
};
use application::{BriefingOutcome, Clock, DashboardSubject, research_dashboard_subject};
use domain::Timestamp;

pub(super) async fn generate<R: Runtime, P: PortTypes>(
    handle: &AppHandle<R>,
    ports: &Ports<P>,
    key: DashboardCacheKey,
    book: Option<&inputs::Book>,
    now: Timestamp,
) -> ResearchSection {
    let data = inputs::collect(handle, ports, key, book, now).await;
    match data {
        Ok(Some((subject, evidence))) => {
            match research_dashboard_subject(ports.ai_provider.as_ref(), &subject, &evidence, now)
                .await
            {
                BriefingOutcome::Completed {
                    body,
                    sources: cited,
                } => ResearchSection {
                    status: ResearchStatus::Ready,
                    as_of: kst_iso(ports.clock.now()),
                    entries: vec![ResearchEntry {
                        symbol: match &subject {
                            DashboardSubject::Candidate(symbol, _) => {
                                Some(symbol.as_str().to_owned())
                            }
                            DashboardSubject::Holdings | DashboardSubject::Economy => None,
                        },
                        display_name: match &subject {
                            DashboardSubject::Candidate(_, name) => Some(name.as_str().to_owned()),
                            DashboardSubject::Holdings | DashboardSubject::Economy => None,
                        },
                        detail: super::ResearchDetail::Unavailable,
                        body: body.as_untrusted_str().to_owned(),
                    }],
                    sources: cited
                        .into_iter()
                        .map(|source| ResearchSource {
                            title: source.title.as_untrusted_str().to_owned(),
                            url: source.url.map(|url| url.as_str().to_owned()),
                            observed_at: kst_iso(source.observed_at),
                        })
                        .collect(),
                },
                BriefingOutcome::Failed(_) => ResearchSection::status(ResearchStatus::Failed),
            }
        }
        Ok(None) => ResearchSection::status(ResearchStatus::Empty),
        Err(()) => ResearchSection::status(ResearchStatus::Failed),
    }
}
