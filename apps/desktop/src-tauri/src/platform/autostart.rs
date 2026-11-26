use application::{DeliveryReport, EventBus};
use domain::DomainEvent;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

pub const LOGIN_ARGUMENT: &str = "--login-start";
pub const AGENT_LABEL: &str = "dev.stockcopilot.desktop.login";
const SETUP_MARKER: &str = "autostart-configured";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchMode {
    Interactive,
    LoginBackground,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchAgentAutostart {
    agent: PathBuf,
    bundle: PathBuf,
}

fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

impl LaunchAgentAutostart {
    #[must_use]
    pub const fn new(agent: PathBuf, bundle: PathBuf) -> Self {
        Self { agent, bundle }
    }

    #[must_use]
    pub fn in_home(home: &Path, bundle: PathBuf) -> Self {
        Self::new(
            home.join("Library")
                .join("LaunchAgents")
                .join(format!("{AGENT_LABEL}.plist")),
            bundle,
        )
    }

    #[must_use]
    pub fn agent_path(&self) -> &Path {
        &self.agent
    }

    #[must_use]
    pub fn plist(&self) -> String {
        let bundle = escaped(&self.bundle.to_string_lossy());
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n\
<dict>\n\
  <key>Label</key>\n\
  <string>{AGENT_LABEL}</string>\n\
  <key>ProgramArguments</key>\n\
  <array>\n\
    <string>/usr/bin/open</string>\n\
    <string>-g</string>\n\
    <string>-a</string>\n\
    <string>{bundle}</string>\n\
    <string>--args</string>\n\
    <string>{LOGIN_ARGUMENT}</string>\n\
  </array>\n\
  <key>RunAtLoad</key>\n\
  <true/>\n\
  <key>LimitLoadToSessionType</key>\n\
  <string>Aqua</string>\n\
</dict>\n\
</plist>\n"
        )
    }
}

fn system_failed(error: &std::io::Error) -> AutostartError {
    AutostartError::SystemFailed(error.kind().to_string())
}

impl AutostartPort for LaunchAgentAutostart {
    fn current(&self) -> Result<AutostartState, AutostartError> {
        match std::fs::metadata(&self.agent) {
            Ok(_found) => Ok(AutostartState::Enabled),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(AutostartState::Disabled)
            }
            Err(error) => Err(system_failed(&error)),
        }
    }

    fn set_state(&self, desired: AutostartState) -> Result<AutostartState, AutostartError> {
        match desired {
            AutostartState::Enabled => {
                let parent = self.agent.parent().ok_or(AutostartError::Unavailable)?;
                std::fs::create_dir_all(parent).map_err(|error| system_failed(&error))?;
                std::fs::write(&self.agent, self.plist()).map_err(|error| system_failed(&error))?;
            }
            AutostartState::Disabled => match std::fs::remove_file(&self.agent) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(system_failed(&error)),
            },
        }
        self.current()
    }
}

pub type LoginItems = AutostartController<LaunchAgentAutostart>;

pub fn prepare_login_items(
    port: LaunchAgentAutostart,
    data_dir: &Path,
) -> Result<LoginItems, AutostartError> {
    let marker = data_dir.join(SETUP_MARKER);
    if std::fs::metadata(&marker).is_ok() {
        let current = port.current()?;
        return Ok(AutostartController::with_initial(port, current));
    }
    let controller = AutostartController::new(port);
    let _synced = controller.reconcile_startup()?;
    std::fs::create_dir_all(data_dir).map_err(|error| system_failed(&error))?;
    std::fs::write(&marker, b"1").map_err(|error| system_failed(&error))?;
    Ok(controller)
}

#[derive(Debug, Clone)]
pub struct LoginItemsSetup {
    pub port: LaunchAgentAutostart,
    pub data_dir: PathBuf,
}

pub struct LoginItemsState {
    pub items: std::sync::Arc<LoginItems>,
    pub events: std::sync::Arc<EventBus>,
}

impl std::fmt::Debug for LoginItemsState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LoginItemsState")
    }
}

pub fn install<R: tauri::Runtime>(
    app: &tauri::App<R>,
    events: &std::sync::Arc<EventBus>,
) -> Option<AutostartState> {
    use tauri::Manager as _;
    let setup = app.try_state::<LoginItemsSetup>()?.inner().clone();
    let items = prepare_login_items(setup.port, &setup.data_dir).ok()?;
    let state = items.desired_state();
    let _managed = app.manage(LoginItemsState {
        items: std::sync::Arc::new(items),
        events: std::sync::Arc::clone(events),
    });
    Some(state)
}

#[must_use]
pub fn launch_mode<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> LaunchMode {
    use tauri::Manager as _;
    app.try_state::<LaunchMode>()
        .map_or(LaunchMode::Interactive, |mode| *mode.inner())
}
