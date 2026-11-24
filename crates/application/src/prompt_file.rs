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
pub struct PromptFile {
    id: String,
    version: String,
    task: String,
    body: String,
}

const KEYS: [&str; 6] = ["id", "version", "task", "inputs", "output_schema", "tools"];
const PROVIDER_TOOL_NAMES: [&str; 9] = [
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

impl PromptFile {
    pub fn parse(source: &str) -> Result<Self, PromptFileError> {
        let rest = source
            .strip_prefix("---\n")
            .ok_or(PromptFileError::MissingFrontMatter)?;
        let (header, body) = rest
            .split_once("\n---\n")
            .ok_or(PromptFileError::MissingFrontMatter)?;
        let mut values: [Option<&str>; 6] = [None; 6];
        for line in header.lines() {
            let (key, raw) = line.split_once(':').ok_or(PromptFileError::MalformedLine)?;
            let index = KEYS
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
        if PROVIDER_TOOL_NAMES.iter().any(|name| body.contains(name)) {
            return Err(PromptFileError::ProviderToolName);
        }
        let field = |index: usize| {
            values
                .get(index)
                .copied()
                .flatten()
                .map(str::to_owned)
                .ok_or(PromptFileError::MissingKey)
        };
        Ok(Self {
            id: field(0)?,
            version: field(1)?,
            task: field(2)?,
            body: body.to_owned(),
        })
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    #[must_use]
    pub fn task(&self) -> &str {
        &self.task
    }

    #[must_use]
    pub fn body(&self) -> &str {
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
    id: String,
    version: String,
    digest: PromptDigest,
}

impl PromptRef {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
    #[must_use]
    pub fn version(&self) -> &str {
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
        task_folder: &str,
        version_file: &str,
        source: &str,
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
