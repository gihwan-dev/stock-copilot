use crate::{SqliteRepositories, shared::opened, sql::dashboard_research as sql};
use application::{
    DashboardCache, DashboardCacheDocument, DashboardCacheKey, SystemPort, SystemPortError,
    WriteCompleted,
};
use rusqlite::OptionalExtension;

const fn tag(key: DashboardCacheKey) -> &'static str {
    match key {
        DashboardCacheKey::Holdings => "holdings",
        DashboardCacheKey::Candidates => "candidates",
        DashboardCacheKey::Economy => "economy",
    }
}

impl DashboardCache for SqliteRepositories {
    async fn dashboard_section(
        &self,
        key: DashboardCacheKey,
    ) -> Result<Option<DashboardCacheDocument>, SystemPortError> {
        let guard = self.shared().read().await;
        opened(guard.as_ref(), SystemPort::Repository)?
            .read(move |db| {
                db.query_row(sql::LOAD, [tag(key)], |row| row.get(0))
                    .optional()
                    .map(|value: Option<String>| value.map(DashboardCacheDocument::new))
            })
            .await
    }

    async fn save_dashboard_section(
        &self,
        key: DashboardCacheKey,
        payload: DashboardCacheDocument,
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared().read().await;
        opened(guard.as_ref(), SystemPort::Repository)?
            .write(move |db| {
                db.execute(sql::SAVE, rusqlite::params![tag(key), payload.as_str()])?;
                Ok(WriteCompleted)
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::{DashboardCache, DashboardCacheDocument, DashboardCacheKey, SqliteRepositories};
    use crate::{
        Database, DatabasePath, SharedDatabase,
        tests::{Fixture, clock, key, runtime},
    };

    #[test]
    fn research_sections_survive_reopen_and_one_failure_does_not_replace_a_sibling() {
        let fixture = Fixture::new();
        let path = DatabasePath::new(fixture.path());
        let rt = runtime();
        rt.block_on(async {
            let shared = SharedDatabase::new();
            let rows = SqliteRepositories::new(shared.clone());
            shared
                .install(Database::open(&path, &key(), &clock()).expect("합성 저장소"))
                .await;
            for (kind, payload) in [
                (DashboardCacheKey::Holdings, "보유 성공"),
                (DashboardCacheKey::Candidates, "후보 실패"),
            ] {
                rows.save_dashboard_section(kind, DashboardCacheDocument::new(payload.to_owned()))
                    .await
                    .expect("개별 저장");
            }
            shared.close().await.expect("닫기");
            shared
                .install(Database::open(&path, &key(), &clock()).expect("재시작"))
                .await;
            assert_eq!(
                rows.dashboard_section(DashboardCacheKey::Holdings)
                    .await
                    .expect("조회")
                    .as_ref()
                    .map(DashboardCacheDocument::as_str),
                Some("보유 성공")
            );
            assert_eq!(
                rows.dashboard_section(DashboardCacheKey::Candidates)
                    .await
                    .expect("조회")
                    .as_ref()
                    .map(DashboardCacheDocument::as_str),
                Some("후보 실패")
            );
            assert_eq!(
                rows.dashboard_section(DashboardCacheKey::Economy)
                    .await
                    .expect("조회")
                    .as_ref()
                    .map(DashboardCacheDocument::as_str),
                None
            );
        });
    }
}
