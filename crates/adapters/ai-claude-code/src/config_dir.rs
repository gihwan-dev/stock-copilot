use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

pub const CONFIG_DIR_ENV_KEY: &str = "CLAUDE_CONFIG_DIR";

#[derive(Debug, Clone, Default)]
pub struct ConfigDirSlot(Arc<RwLock<Option<PathBuf>>>);

impl ConfigDirSlot {
    pub fn set(&self, dir: Option<&Path>) {
        let chosen = dir
            .filter(|path| path.is_absolute() && !path.as_os_str().is_empty())
            .map(Path::to_path_buf);
        match self.0.write() {
            Ok(mut guard) => *guard = chosen,
            Err(poisoned) => *poisoned.into_inner() = chosen,
        }
    }

    #[must_use]
    pub fn get(&self) -> Option<PathBuf> {
        match self.0.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::ConfigDirSlot;

    #[test]
    fn only_an_absolute_chosen_directory_is_kept() {
        let slot = ConfigDirSlot::default();
        assert_eq!(slot.get(), None);
        slot.set(Some(Path::new("/Users/me/.claude-second")));
        assert_eq!(
            slot.get().as_deref(),
            Some(Path::new("/Users/me/.claude-second"))
        );
        slot.set(Some(Path::new("relative")));
        assert_eq!(slot.get(), None);
        slot.set(Some(Path::new("/x")));
        slot.set(None);
        assert_eq!(slot.get(), None);
    }
}
