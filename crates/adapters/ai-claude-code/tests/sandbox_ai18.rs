#[cfg(test)]
mod tests {
    use ai_claude_code::{
        ClaudeCodeConfig, HostEnvironment, McpBridgeConfig, ProbeExit, SYSTEM_SANDBOX_EXEC,
        SandboxLayer, SandboxProbeOutcome, SandboxProfile, SandboxProfileError,
        classify_sandbox_probe, probe_sandbox_layer,
    };
    use application::{
        AiRunId, AnalysisPrompt, ModelSelection, ReasoningEffort, RunInput, RunPurpose, RunSpec,
        SecretValue,
    };
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(1_u64);

    struct TempDirGuard(PathBuf);

    impl TempDirGuard {
        fn new(label: &str) -> Self {
            let seq = COUNTER.fetch_add(1_u64, Ordering::Relaxed);
            let path =
                PathBuf::from("/tmp").join(format!("sc-ai18-{}-{seq}-{label}", std::process::id()));
            fs::create_dir_all(&path).expect("create dir");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            match fs::remove_dir_all(&self.0) {
                Ok(()) | Err(_) => {}
            }
        }
    }

    fn profile() -> SandboxProfile {
        SandboxProfile::try_new(&[
            PathBuf::from("/Users/someone/Library/Application Support/stock-copilot"),
            PathBuf::from("/private/var/app-data"),
        ])
        .expect("profile")
    }

    fn config(executable: &Path) -> ClaudeCodeConfig {
        let bridge = McpBridgeConfig::try_new(
            String::from("app"),
            PathBuf::from("/usr/bin/true"),
            Vec::new(),
        )
        .expect("bridge");
        ClaudeCodeConfig::new(
            executable.to_path_buf(),
            PathBuf::from("/tmp/sc-ai18-work"),
            String::from("app"),
            BTreeSet::from([String::from("mcp__app__get_portfolio")]),
            BTreeSet::from([String::from("cc-plugin-privacy")]),
        )
        .expect("config")
        .with_host_environment(HostEnvironment::new_clean(
            String::from("/tmp/fake-home"),
            String::from("/usr/bin:/bin"),
            String::from("testuser"),
            Some(String::from("testuser")),
            String::from("/tmp"),
        ))
        .with_mcp_bridge(bridge, SecretValue::new(b"ai18-token".to_vec()))
    }

    fn spec() -> RunSpec {
        RunSpec::new(
            AiRunId::try_new("run-ai18").expect("id"),
            RunInput::Analysis(AnalysisPrompt::try_new("hello").expect("prompt")),
            ModelSelection::Default,
            ReasoningEffort::Medium,
            RunPurpose::Chat,
        )
    }

    #[test]
    fn profile_denies_each_path_on_top_of_default_allow() {
        assert_eq!(
            profile().text(),
            "(version 1)\n(allow default)\n\
             (deny file-read* file-write* (subpath \"/Users/someone/Library/Application Support/stock-copilot\"))\n\
             (deny file-read* file-write* (subpath \"/private/var/app-data\"))\n"
        );
        assert_eq!(profile().denied_paths().len(), 2);
    }

    #[test]
    fn profile_rejects_paths_that_could_escape_the_literal() {
        let cases = [
            (Vec::new(), SandboxProfileError::NoDeniedPaths),
            (
                vec![PathBuf::from("relative/data")],
                SandboxProfileError::RelativePath,
            ),
            (
                vec![PathBuf::from("/a\"))(allow default")],
                SandboxProfileError::UnsafeCharacter,
            ),
            (
                vec![PathBuf::from("/a\\b")],
                SandboxProfileError::UnsafeCharacter,
            ),
            (
                vec![PathBuf::from("/a\n(allow file-read*)")],
                SandboxProfileError::UnsafeCharacter,
            ),
            (vec![PathBuf::from("/")], SandboxProfileError::RootPath),
            (
                vec![PathBuf::from("/ok"), PathBuf::from("bad")],
                SandboxProfileError::RelativePath,
            ),
        ];
        for (paths, expected) in cases {
            assert_eq!(SandboxProfile::try_new(&paths), Err(expected), "{paths:?}");
        }
    }

    #[test]
    fn sandbox_wraps_the_same_isolated_command_without_new_env() {
        let executable = PathBuf::from("/opt/claude/bin/claude");
        let plain = config(&executable)
            .preview_command_plan(&spec(), Path::new("/tmp/w"), Path::new("/tmp/m.json"))
            .expect("plain");
        let wrapped = config(&executable)
            .with_sandbox(SandboxLayer::system(profile()))
            .preview_command_plan(&spec(), Path::new("/tmp/w"), Path::new("/tmp/m.json"))
            .expect("wrapped");
        assert_eq!(wrapped.executable(), Path::new(SYSTEM_SANDBOX_EXEC));
        let mut expected = vec![
            String::from("-p"),
            String::from(profile().text()),
            String::from("/opt/claude/bin/claude"),
        ];
        expected.extend(plain.args().iter().cloned());
        assert_eq!(wrapped.args(), expected.as_slice());
        assert_eq!(wrapped.env_keys(), plain.env_keys(), "환경 변수는 그대로");
        assert_eq!(wrapped.work_dir(), plain.work_dir());
        let removed = config(&executable)
            .with_sandbox(SandboxLayer::system(profile()))
            .without_sandbox()
            .preview_command_plan(&spec(), Path::new("/tmp/w"), Path::new("/tmp/m.json"))
            .expect("removed");
        assert_eq!(removed.executable(), executable.as_path());
        assert_eq!(removed.args(), plain.args(), "겹을 빼도 격리 인자는 같다");
    }

    #[test]
    fn probe_classification_requires_allowed_read_and_denied_failure() {
        let both = "sandbox-probe-allowedsandbox-probe-denied";
        let cases = [
            (
                ProbeExit::Failure,
                "sandbox-probe-allowed",
                SandboxProbeOutcome::Applied,
            ),
            (ProbeExit::Success, both, SandboxProbeOutcome::NotApplied),
            (ProbeExit::Failure, both, SandboxProbeOutcome::NotApplied),
            (
                ProbeExit::Failure,
                "sandbox-probe-denied",
                SandboxProbeOutcome::NotApplied,
            ),
            (
                ProbeExit::Success,
                "sandbox-probe-allowed",
                SandboxProbeOutcome::Unavailable,
            ),
            (ProbeExit::Failure, "", SandboxProbeOutcome::Unavailable),
            (ProbeExit::Success, "", SandboxProbeOutcome::Unavailable),
        ];
        for (exit, stdout, expected) in cases {
            assert_eq!(
                classify_sandbox_probe(exit, stdout),
                expected,
                "{exit:?} {stdout}"
            );
        }
    }

    #[test]
    fn probe_reports_unavailable_when_sandbox_does_not_run_and_cleans_scratch() {
        let dir = TempDirGuard::new("probe");
        for sandbox_exec in [
            PathBuf::from("/usr/bin/true"),
            PathBuf::from("/usr/bin/false"),
            dir.path().join("missing-sandbox-exec"),
        ] {
            let layer = SandboxLayer::new(sandbox_exec.clone(), profile());
            assert_eq!(
                probe_sandbox_layer(&layer, dir.path()),
                SandboxProbeOutcome::Unavailable,
                "{}",
                sandbox_exec.display()
            );
            let leftovers: Vec<PathBuf> = fs::read_dir(dir.path())
                .expect("scratch")
                .map(|entry| entry.expect("entry").path())
                .collect();
            assert_eq!(leftovers, Vec::<PathBuf>::new(), "점검 폴더는 남지 않는다");
        }
    }
}
