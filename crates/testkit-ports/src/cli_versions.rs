use crate::script::State;
use application::{
    CliVersion, CliVersionSource, SystemPortError, VerifiedCliVersions, WriteCompleted,
};
use std::collections::VecDeque;

#[derive(Debug, Default)]
pub struct MemoryVerifiedCliVersions {
    versions: State<Vec<CliVersion>>,
    failures: State<VecDeque<SystemPortError>>,
}

impl MemoryVerifiedCliVersions {
    #[must_use]
    pub fn with_versions(versions: Vec<CliVersion>) -> Self {
        Self {
            versions: State::new(versions),
            failures: State::default(),
        }
    }
    pub fn fail_next(&self, error: SystemPortError) {
        self.failures.write().push_back(error);
    }
    #[must_use]
    pub fn versions(&self) -> Vec<CliVersion> {
        self.versions.write().clone()
    }
}

impl VerifiedCliVersions for MemoryVerifiedCliVersions {
    fn verified(&self) -> impl Future<Output = Result<Vec<CliVersion>, SystemPortError>> + Send {
        let failure = self.failures.write().pop_front();
        std::future::ready(match failure {
            Some(error) => Err(error),
            None => Ok(self.versions.write().clone()),
        })
    }

    fn record_verified(
        &self,
        version: &CliVersion,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        let failure = self.failures.write().pop_front();
        if let Some(error) = failure {
            return std::future::ready(Err(error));
        }
        let mut versions = self.versions.write();
        if !versions.contains(version) {
            versions.push(version.clone());
        }
        std::future::ready(Ok(WriteCompleted))
    }
}

#[derive(Debug, Default)]
pub struct ScriptedCliVersionSource {
    answers: State<VecDeque<Option<CliVersion>>>,
}

impl ScriptedCliVersionSource {
    pub fn push(&self, answer: Option<CliVersion>) {
        self.answers.write().push_back(answer);
    }
}

impl CliVersionSource for ScriptedCliVersionSource {
    fn observed_version(&self) -> impl Future<Output = Option<CliVersion>> + Send {
        let answer = self.answers.write().pop_front().flatten();
        std::future::ready(answer)
    }
}
