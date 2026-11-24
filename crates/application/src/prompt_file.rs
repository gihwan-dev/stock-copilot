#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptFileError {
    MissingFrontMatter,
    MalformedLine,
    UnknownKey,
    DuplicateKey,
    MissingKey,
    EmptyValue,
    EmptyBody,
    ProviderToolName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptSource(String);

impl PromptSource {
    #[must_use]
    pub fn new(value: &str) -> Self {
        Self(value.to_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptId(String);

impl PromptId {
    #[must_use]
    pub fn new(value: &str) -> Self {
        Self(value.to_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptVersion(String);

impl PromptVersion {
    #[must_use]
    pub fn new(value: &str) -> Self {
        Self(value.to_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptTask(String);

impl PromptTask {
    #[must_use]
    pub fn new(value: &str) -> Self {
        Self(value.to_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptBody(String);

impl PromptBody {
    #[must_use]
    pub fn new(value: &str) -> Self {
        Self(value.to_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptFile {
    id: PromptId,
    version: PromptVersion,
    task: PromptTask,
    body: PromptBody,
}

impl PromptFile {
    pub fn parse(source: &PromptSource) -> Result<Self, PromptFileError> {
        let keys = ["id", "version", "task", "inputs", "output_schema", "tools"];
        let provider_tool_names = [
            "mcp__",
            "WebSearch",
            "WebFetch",
            "Bash(",
            "TodoWrite",
            "web_search_preview",
            "apply_patch",
            "functions.",
            "computer_use",
        ];
        let rest = source
            .as_str()
            .strip_prefix("---\n")
            .ok_or(PromptFileError::MissingFrontMatter)?;
        let (header, body) = rest
            .split_once("\n---\n")
            .ok_or(PromptFileError::MissingFrontMatter)?;
        let mut values: [Option<&str>; 6] = [None; 6];
        for line in header.lines() {
            let (key, raw) = line.split_once(':').ok_or(PromptFileError::MalformedLine)?;
            let index = keys
                .iter()
                .position(|known| *known == key)
                .ok_or(PromptFileError::UnknownKey)?;
            let value = raw.trim();
            if value.is_empty() {
                return Err(PromptFileError::EmptyValue);
            }
            let slot = values.get_mut(index).ok_or(PromptFileError::UnknownKey)?;
            if slot.replace(value).is_some() {
                return Err(PromptFileError::DuplicateKey);
            }
        }
        if values.iter().any(Option::is_none) {
            return Err(PromptFileError::MissingKey);
        }
        if body.trim().is_empty() {
            return Err(PromptFileError::EmptyBody);
        }
        if provider_tool_names.iter().any(|name| body.contains(name)) {
            return Err(PromptFileError::ProviderToolName);
        }
        let [Some(id), Some(version), Some(task), ..] = values else {
            return Err(PromptFileError::MissingKey);
        };
        Ok(Self {
            id: PromptId::new(id),
            version: PromptVersion::new(version),
            task: PromptTask::new(task),
            body: PromptBody::new(body),
        })
    }

    #[must_use]
    pub const fn id(&self) -> &PromptId {
        &self.id
    }

    #[must_use]
    pub const fn version(&self) -> &PromptVersion {
        &self.version
    }

    #[must_use]
    pub const fn task(&self) -> &PromptTask {
        &self.task
    }

    #[must_use]
    pub const fn body(&self) -> &PromptBody {
        &self.body
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptLocationError {
    Format(PromptFileError),
    TaskFolderMismatch,
    VersionFileMismatch,
    InvalidDigest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptDigest(String);

impl PromptDigest {
    pub fn try_from_hex(value: &str) -> Result<Self, PromptLocationError> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(PromptLocationError::InvalidDigest);
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptRef {
    id: PromptId,
    version: PromptVersion,
    digest: PromptDigest,
}

impl PromptRef {
    #[must_use]
    pub const fn id(&self) -> &PromptId {
        &self.id
    }
    #[must_use]
    pub const fn version(&self) -> &PromptVersion {
        &self.version
    }
    #[must_use]
    pub const fn digest(&self) -> &PromptDigest {
        &self.digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedPrompt {
    file: PromptFile,
    digest: PromptDigest,
}

impl LoadedPrompt {
    pub fn load(
        task_folder: &PromptTask,
        version_file: &PromptVersion,
        source: &PromptSource,
        digest: PromptDigest,
    ) -> Result<Self, PromptLocationError> {
        let file = PromptFile::parse(source).map_err(PromptLocationError::Format)?;
        if file.task() != task_folder {
            return Err(PromptLocationError::TaskFolderMismatch);
        }
        if file.version() != version_file {
            return Err(PromptLocationError::VersionFileMismatch);
        }
        Ok(Self { file, digest })
    }
    #[must_use]
    pub const fn file(&self) -> &PromptFile {
        &self.file
    }
    #[must_use]
    pub fn prompt_ref(&self) -> PromptRef {
        PromptRef {
            id: self.file.id.clone(),
            version: self.file.version.clone(),
            digest: self.digest.clone(),
        }
    }
}
