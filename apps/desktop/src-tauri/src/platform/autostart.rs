use application::{DeliveryReport, EventBus};
use domain::DomainEvent;
use std::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AutostartState {
    #[default]
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutostartError {
    Unavailable,
    SystemFailed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutostartSyncOutcome {
    Synchronized(AutostartState),
    Unchanged(AutostartState),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutostartChangeReport {
    pub state: AutostartState,
    pub delivery: DeliveryReport,
}

pub trait AutostartPort: Send + Sync {
    fn current(&self) -> Result<AutostartState, AutostartError>;
    fn set_state(&self, desired: AutostartState) -> Result<AutostartState, AutostartError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct UnavailableAutostartPort;

impl AutostartPort for UnavailableAutostartPort {
    fn current(&self) -> Result<AutostartState, AutostartError> {
        Err(AutostartError::Unavailable)
    }

    fn set_state(&self, _desired: AutostartState) -> Result<AutostartState, AutostartError> {
        Err(AutostartError::Unavailable)
    }
}

#[derive(Debug)]
pub struct AutostartController<P> {
    port: P,
    desired: RwLock<AutostartState>,
}

impl<P> AutostartController<P> {
    #[must_use]
    pub const fn new(port: P) -> Self {
        Self::with_initial(port, AutostartState::Enabled)
    }

    #[must_use]
    pub const fn with_initial(port: P, initial: AutostartState) -> Self {
        Self {
            port,
            desired: RwLock::new(initial),
        }
    }

    #[must_use]
    pub fn desired_state(&self) -> AutostartState {
        if let Ok(guard) = self.desired.read() {
            *guard
        } else {
            AutostartState::Enabled
        }
    }

    #[must_use]
    pub const fn port(&self) -> &P {
        &self.port
    }
}

impl<P: AutostartPort> AutostartController<P> {
    pub fn reconcile_startup(&self) -> Result<AutostartSyncOutcome, AutostartError> {
        let target = self.desired_state();
        let actual = self.port.current()?;
        if actual == target {
            return Ok(AutostartSyncOutcome::Unchanged(actual));
        }
        let applied = self.port.set_state(target)?;
        Ok(AutostartSyncOutcome::Synchronized(applied))
    }

    pub fn apply(
        &self,
        next: AutostartState,
        events: &EventBus,
    ) -> Result<AutostartChangeReport, AutostartError> {
        let previous = self.desired_state();
        if previous == next {
            let current = self.port.current()?;
            if current == next {
                return Ok(AutostartChangeReport {
                    state: current,
                    delivery: DeliveryReport::default(),
                });
            }
        }
        let applied = self.port.set_state(next)?;
        let mut guard = self
            .desired
            .write()
            .map_err(|_error| AutostartError::Unavailable)?;
        *guard = applied;
        let delivery = events.publish(DomainEvent::SettingsChanged);
        Ok(AutostartChangeReport {
            state: applied,
            delivery,
        })
    }
}
