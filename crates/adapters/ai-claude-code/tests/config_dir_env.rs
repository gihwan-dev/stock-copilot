#[cfg(test)]
mod tests {
    use ai_claude_code::{ClaudeCodeConfig, ConfigDirSlot, HostEnvironment};
    use application::{
        AiRunId, CollectionQuery, CollectionTopic, ModelSelection, ReasoningEffort, RunInput,
        RunPurpose, RunSpec,
    };
    use domain::Symbol;
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    fn config(slot: ConfigDirSlot) -> ClaudeCodeConfig {
        ClaudeCodeConfig::new(
            PathBuf::from("/usr/local/bin/claude"),
            PathBuf::from("/tmp/w"),
            String::from("app"),
            BTreeSet::new(),
            BTreeSet::new(),
        )
        .expect("config")
        .with_host_environment(HostEnvironment::from_pairs([
            ("HOME", "/tmp/sandbox-home"),
            ("PATH", "/usr/bin:/bin"),
            ("USER", "localuser"),
            ("TMPDIR", "/tmp"),
            ("CLAUDE_CONFIG_DIR", "/tmp/should-not-forward"),
        ]))
        .with_collection_json_schema(String::from(ai_claude_code::WEB_COLLECTION_JSON_SCHEMA))
        .with_config_dir_slot(slot)
    }

    fn spec() -> RunSpec {
        RunSpec::new(
            AiRunId::try_new("run-config-dir").expect("id"),
            RunInput::Collection(CollectionQuery::CodeTopics {
                instruments: vec![Symbol::try_new("005930").expect("sym")],
                topics: vec![CollectionTopic::CompanyNews],
            }),
            ModelSelection::Fast,
            ReasoningEffort::Low,
            RunPurpose::WebCollection,
        )
    }

    fn keys(config: &ClaudeCodeConfig) -> Vec<String> {
        config
            .preview_command_plan(
                &spec(),
                Path::new("/tmp/w/work"),
                Path::new("/tmp/w/m.json"),
            )
            .expect("plan")
            .env_keys()
            .into_iter()
            .map(String::from)
            .collect()
    }

    #[test]
    fn the_host_config_dir_is_never_forwarded_but_a_chosen_one_is_set_once() {
        let slot = ConfigDirSlot::default();
        let chosen = config(slot.clone());
        assert!(!keys(&chosen).contains(&String::from("CLAUDE_CONFIG_DIR")));
        slot.set(Some(Path::new("/Users/me/.claude-second")));
        assert_eq!(
            keys(&chosen)
                .iter()
                .filter(|key| key.as_str() == "CLAUDE_CONFIG_DIR")
                .count(),
            1
        );
        slot.set(None);
        assert!(!keys(&chosen).contains(&String::from("CLAUDE_CONFIG_DIR")));
    }
}
