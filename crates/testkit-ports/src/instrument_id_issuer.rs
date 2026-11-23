use crate::Script;
use application::{InstrumentIdIssuer, SystemPortError};
use domain::IssuedInstrumentId;

#[derive(Debug, Default)]
pub struct ScriptedInstrumentIdIssuer {
    script: Script<(), Result<IssuedInstrumentId, SystemPortError>>,
}

impl ScriptedInstrumentIdIssuer {
    pub fn push(&self, result: Result<IssuedInstrumentId, SystemPortError>) {
        self.script.push(result);
    }

    #[must_use]
    pub fn calls(&self) -> Vec<()> {
        self.script.calls()
    }
}

impl InstrumentIdIssuer for ScriptedInstrumentIdIssuer {
    async fn issue(&self) -> Result<IssuedInstrumentId, SystemPortError> {
        self.script.run(()).await
    }
}
