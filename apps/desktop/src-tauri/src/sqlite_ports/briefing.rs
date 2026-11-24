use super::{REPOSITORY, SqliteRepositories, opened};
use application::{BriefingRecord, BriefingRepository, SystemPortError, WriteCompleted};
use domain::BriefingId;
use store_sqlite::BriefingStore;

impl BriefingRepository for SqliteRepositories {
    async fn save(&self, record: &BriefingRecord) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        BriefingRepository::save(&BriefingStore::new(database), record).await
    }
    async fn find(&self, id: &BriefingId) -> Result<Option<BriefingRecord>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        BriefingRepository::find(&BriefingStore::new(database), id).await
    }
    async fn recent(&self) -> Result<Vec<BriefingRecord>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        BriefingRepository::recent(&BriefingStore::new(database)).await
    }
}
