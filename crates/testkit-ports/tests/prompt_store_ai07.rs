#[cfg(test)]
mod tests {
    use application::{
        AiJobFinishRecord, AiJobOutcome, AiJobRecordStore, AiJobRun, AiJobTiming, LoadedPrompt,
        PromptDigest, PromptFileError, PromptLocationError, PromptSource, PromptTask,
        PromptVersion, RunKind, RunPurpose, TokenUsage,
    };
    use domain::{AiJobId, Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
    use testkit_ports::MemoryAiJobRecords;

    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
        {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("expected ready"),
        }
    }
    fn at(value: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(value),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("time")
    }
    fn estimate_source() -> PromptSource {
        PromptSource::new(
            &std::fs::read_to_string("../../prompts/estimate/v1.md").expect("추정 프롬프트"),
        )
    }
    fn digest() -> PromptDigest {
        PromptDigest::try_from_hex(DIGEST).expect("해시")
    }

    #[test]
    fn every_prompt_file_loads_from_its_task_folder_and_version_file() {
        let mut loaded = 0_u32;
        for entry in std::fs::read_dir("../../prompts").expect("폴더") {
            let task = entry.expect("작업").path();
            let task_name = task
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .expect("이름")
                .to_owned();
            for file in std::fs::read_dir(&task).expect("버전") {
                let path = file.expect("파일").path();
                assert_eq!(
                    path.extension().and_then(std::ffi::OsStr::to_str),
                    Some("md")
                );
                let version = path
                    .file_stem()
                    .and_then(std::ffi::OsStr::to_str)
                    .expect("버전");
                let source = std::fs::read_to_string(&path).expect("본문");
                LoadedPrompt::load(
                    &PromptTask::new(&task_name),
                    &PromptVersion::new(version),
                    &PromptSource::new(&source),
                    digest(),
                )
                .expect("위치와 형식");
                loaded = loaded.saturating_add(1);
            }
        }
        assert_ne!(loaded, 0_u32);
    }

    #[test]
    fn misplaced_or_malformed_prompts_are_rejected() {
        let source = estimate_source();
        assert_eq!(
            LoadedPrompt::load(
                &PromptTask::new("briefing"),
                &PromptVersion::new("v1"),
                &source,
                digest()
            ),
            Err(PromptLocationError::TaskFolderMismatch)
        );
        assert_eq!(
            LoadedPrompt::load(
                &PromptTask::new("estimate"),
                &PromptVersion::new("v2"),
                &source,
                digest()
            ),
            Err(PromptLocationError::VersionFileMismatch)
        );
        assert_eq!(
            LoadedPrompt::load(
                &PromptTask::new("estimate"),
                &PromptVersion::new("v1"),
                &PromptSource::new("본문만"),
                digest()
            ),
            Err(PromptLocationError::Format(
                PromptFileError::MissingFrontMatter
            ))
        );
        for bad in [
            "",
            "ABCDEF",
            DIGEST.get(1..).expect("63자리"),
            &format!("{}G", DIGEST.get(1..).expect("63자리")),
            &DIGEST.to_uppercase(),
        ] {
            assert_eq!(
                PromptDigest::try_from_hex(bad),
                Err(PromptLocationError::InvalidDigest),
                "{bad}"
            );
        }
    }

    #[test]
    fn job_record_keeps_prompt_id_version_and_digest() {
        let loaded = LoadedPrompt::load(
            &PromptTask::new("estimate"),
            &PromptVersion::new("v1"),
            &estimate_source(),
            digest(),
        )
        .expect("로드");
        let reference = loaded.prompt_ref();
        assert_eq!(
            (
                reference.id().as_str(),
                reference.version().as_str(),
                reference.digest().as_str()
            ),
            ("estimate", "v1", DIGEST)
        );
        let record = AiJobFinishRecord::new(
            AiJobRun::new(
                AiJobId::try_new("est-1").expect("ID"),
                RunKind::Analysis,
                RunPurpose::Estimate,
            ),
            AiJobOutcome::Completed,
            AiJobTiming::try_new(at(1), at(2)).expect("시간"),
            TokenUsage::zero(),
        )
        .with_prompt(reference.clone());
        let store = MemoryAiJobRecords::default();
        run(store.store_finished(&record)).expect("저장");
        let stored = run(store.finished(record.job()))
            .expect("조회")
            .expect("기록");
        assert_eq!(stored.prompt(), Some(&reference));
    }
}
