use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;
use tauri::{AppHandle, Runtime};

const MAX_SOCKET_PATH_BYTES: usize = 104;
const ACTIVATE_MESSAGE: &str = "ACTIVATE\n";
const SHUTDOWN_MESSAGE: &str = "SHUTDOWN\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocketPath(PathBuf);

impl SocketPath {
    pub fn try_new(path: PathBuf) -> Result<Self, SingleInstanceError> {
        let encoded = path.as_os_str().as_encoded_bytes();
        if encoded.len() > MAX_SOCKET_PATH_BYTES {
            return Err(SingleInstanceError::PathTooLong {
                length: encoded.len(),
            });
        }
        if path.parent().is_none() {
            return Err(SingleInstanceError::MissingParentDirectory);
        }
        Ok(Self(path))
    }

    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SingleInstanceError {
    PathTooLong { length: usize },
    MissingParentDirectory,
    DirectorySetup(String),
    BindFailed(String),
    SignalFailed(String),
}

#[derive(Debug)]
pub enum InstanceAcquisition {
    Primary(SingleInstanceGuard),
    SecondaryNotified,
}

pub struct SingleInstanceGuard {
    path: PathBuf,
    listener_thread: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for SingleInstanceGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SingleInstanceGuard")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl SingleInstanceGuard {
    #[must_use]
    pub fn socket_path(&self) -> &Path {
        &self.path
    }
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        if let Ok(mut stream) = UnixStream::connect(&self.path) {
            if let Err(_error) = stream.write_all(SHUTDOWN_MESSAGE.as_bytes()) {}
            if let Err(_error) = stream.flush() {}
        }
        if let Some(handle) = self.listener_thread.take()
            && let Err(_error) = handle.join()
        {}
        if let Err(_error) = fs::remove_file(&self.path) {}
    }
}

pub fn acquire<Activate>(
    socket_path: SocketPath,
    on_activate: Activate,
) -> Result<InstanceAcquisition, SingleInstanceError>
where
    Activate: Fn() + Send + Sync + 'static,
{
    let path = socket_path.0;
    if try_notify_existing(&path)? {
        return Ok(InstanceAcquisition::SecondaryNotified);
    }
    prepare_socket_directory(&path)?;
    match bind_socket(&path) {
        Ok(listener) => {
            let callback = Arc::new(on_activate);
            let thread = spawn_listener(listener, callback);
            Ok(InstanceAcquisition::Primary(SingleInstanceGuard {
                path,
                listener_thread: Some(thread),
            }))
        }
        Err(error) if error.kind() == ErrorKind::AddrInUse => {
            if try_notify_existing(&path)? {
                return Ok(InstanceAcquisition::SecondaryNotified);
            }
            remove_stale_socket(&path)?;
            let listener = bind_socket(&path)
                .map_err(|bind_err| SingleInstanceError::BindFailed(bind_err.to_string()))?;
            let callback = Arc::new(on_activate);
            let thread = spawn_listener(listener, callback);
            Ok(InstanceAcquisition::Primary(SingleInstanceGuard {
                path,
                listener_thread: Some(thread),
            }))
        }
        Err(error) => Err(SingleInstanceError::BindFailed(error.to_string())),
    }
}

pub fn attach_to_app<R: Runtime>(
    app: &AppHandle<R>,
    socket_path: SocketPath,
) -> Result<InstanceAcquisition, SingleInstanceError> {
    let handle = app.clone();
    acquire(socket_path, move || {
        let target = handle.clone();
        if let Err(_error) = handle.run_on_main_thread(move || {
            if let Err(_show_error) = super::residency::show_and_focus_main(&target) {}
        }) {}
    })
}

fn try_notify_existing(path: &Path) -> Result<bool, SingleInstanceError> {
    match UnixStream::connect(path) {
        Ok(mut stream) => {
            stream
                .write_all(ACTIVATE_MESSAGE.as_bytes())
                .map_err(|error| SingleInstanceError::SignalFailed(error.to_string()))?;
            stream
                .flush()
                .map_err(|error| SingleInstanceError::SignalFailed(error.to_string()))?;
            Ok(true)
        }
        Err(error) if error.kind() == ErrorKind::PermissionDenied => {
            Err(SingleInstanceError::SignalFailed(error.to_string()))
        }
        Err(_error) => Ok(false),
    }
}

fn prepare_socket_directory(path: &Path) -> Result<(), SingleInstanceError> {
    let parent = path
        .parent()
        .ok_or(SingleInstanceError::MissingParentDirectory)?;
    fs::create_dir_all(parent)
        .map_err(|error| SingleInstanceError::DirectorySetup(error.to_string()))?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
        .map_err(|error| SingleInstanceError::DirectorySetup(error.to_string()))
}

fn remove_stale_socket(path: &Path) -> Result<(), SingleInstanceError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(SingleInstanceError::BindFailed(error.to_string())),
    }
}

fn bind_socket(path: &Path) -> std::io::Result<UnixListener> {
    let listener = UnixListener::bind(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

fn spawn_listener(
    listener: UnixListener,
    on_activate: Arc<dyn Fn() + Send + Sync>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        for connection in listener.incoming() {
            let Ok(stream) = connection else {
                break;
            };
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                continue;
            }
            let command = line.trim();
            if command == "SHUTDOWN" {
                break;
            }
            if command == "ACTIVATE" {
                on_activate();
            }
        }
    })
}
