#[cfg(test)]
mod tests {
    use application::{PromptFile, PromptFileError};

    fn estimate() -> String {
        std::fs::read_to_string("../../prompts/estimate/v1.md").expect("추정 프롬프트")
    }

    fn position(body: &str, needle: &str) -> usize {
        body.find(needle).expect(needle)
    }

    #[test]
    fn every_prompt_file_passes_the_format() {
        let tasks = std::fs::read_dir("../../prompts").expect("프롬프트 폴더");
        let mut checked = Vec::new();
        for entry in tasks {
            let task = entry.expect("작업 폴더").path();
            for file in std::fs::read_dir(&task).expect("버전 목록") {
                let path = file.expect("파일").path();
                let source = std::fs::read_to_string(&path).expect("프롬프트");
                let prompt = PromptFile::parse(&source).expect("형식");
                assert_eq!(
                    path.file_stem().and_then(|stem| stem.to_str()),
                    Some(prompt.version())
                );
                checked.push(path);
            }
        }
        assert_ne!(checked, Vec::<std::path::PathBuf>::new());
    }

    #[test]
    fn estimate_prompt_passes_the_file_format() {
        let prompt = PromptFile::parse(&estimate()).expect("형식");
        assert_eq!(prompt.id(), "estimate");
        assert_eq!(prompt.version(), "v1");
        assert_eq!(prompt.task(), "estimate");
    }

    #[test]
    fn estimate_prompt_shows_the_base_probability_before_moving_it() {
        let prompt = PromptFile::parse(&estimate()).expect("형식");
        let body = prompt.body();
        assert!(position(body, "기준 확률 p₀") < position(body, "근거가 있을 때만"));
        assert!(body.contains("근거가 없으면 p₀를 그대로 낸다"));
        assert!(body.contains("반대 논거를 하나 이상"));
        assert!(body.contains("근거 하나마다"));
        assert!(body.contains("`tool:<요청 ID>`"));
        for field in ["task_id", "`p`", "rationale", "counterargument", "sources"] {
            assert!(body.contains(field), "{field}");
        }
    }

    #[test]
    fn malformed_prompt_files_are_rejected() {
        let valid_header = "id: a\nversion: v1\ntask: a\ninputs: x\noutput_schema: y\ntools: z\n";
        for (source, error) in [
            (String::from("본문만"), PromptFileError::MissingFrontMatter),
            (
                format!("---\n{valid_header}extra: 1\n---\n본문"),
                PromptFileError::UnknownKey,
            ),
            (
                format!("---\n{valid_header}id: b\n---\n본문"),
                PromptFileError::DuplicateKey,
            ),
            (
                String::from("---\nid: a\nversion: v1\n---\n본문"),
                PromptFileError::MissingKey,
            ),
            (
                String::from(
                    "---\nid:\nversion: v1\ntask: a\ninputs: x\noutput_schema: y\ntools: z\n---\n본문",
                ),
                PromptFileError::EmptyValue,
            ),
            (
                String::from("---\nid a\n---\n본문"),
                PromptFileError::MalformedLine,
            ),
            (
                format!("---\n{valid_header}---\n  \n"),
                PromptFileError::EmptyBody,
            ),
            (
                format!("---\n{valid_header}---\nmcp__app__submit_estimate를 부른다"),
                PromptFileError::ProviderToolName,
            ),
            (
                format!("---\n{valid_header}---\nWebSearch로 찾는다"),
                PromptFileError::ProviderToolName,
            ),
        ] {
            assert_eq!(
                PromptFile::parse(&source).map(|_file| ()),
                Err(error),
                "{source}"
            );
        }
    }
}
