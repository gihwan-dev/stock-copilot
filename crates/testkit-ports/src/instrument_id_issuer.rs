use crate::Script;
use application::{InstrumentIdIssuer, SystemPortError};
use domain::InstrumentId;

#[derive(Debug, Default)]
pub struct ScriptedInstrumentIdIssuer {
    script: Script<(), Result<InstrumentId, SystemPortError>>,
}

impl ScriptedInstrumentIdIssuer {
    pub fn push(&self, result: Result<InstrumentId, SystemPortError>) {
        self.script.push(result);
    }

    #[must_use]
    pub fn calls(&self) -> Vec<()> {
        self.script.calls()
    }
}

impl InstrumentIdIssuer for ScriptedInstrumentIdIssuer {
    async fn issue(&self) -> Result<InstrumentId, SystemPortError> {
        self.script.run(()).await
    }
}
