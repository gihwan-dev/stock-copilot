#[cfg(test)]
mod tests {
    use application::{
        AiCheckObservation, AiEnvironmentProblem, AiUsage, BugDiagnostic, CheckState,
        ClaudeInstallProblem, ClaudeLoginProblem, CredentialSource, DuplicateKeyProblem,
        ErrorClass, FeedSource, InformationError, IpProblem, IsolationFailure, MissingData,
        NewsCheckState, OrderPrerequisiteProblem, ResetTime, ResponseMismatch, RetryDelay,
        SelfCheckAction, SelfCheckChange, SelfCheckInput, SourceCheckRecord, TossCheckObservation,
        TossKeyProblem, UserAction, evaluate_self_check, isolation_check_observation,
        self_check_change, toss_check_observation,
    };
    use domain::{Nanosecond, OffsetSeconds, SpanSeconds, TimeSpan, Timestamp, UnixSeconds};
    use proptest::{array, prop_assert_eq, proptest, test_runner::Config};

    fn healthy() -> SelfCheckInput {
        SelfCheckInput {
            toss_key: CheckState::Healthy,
            allowed_ip: CheckState::Healthy,
            duplicate_key_use: CheckState::Healthy,
            claude_install: CheckState::Healthy,
            claude_login: CheckState::Healthy,
            ai_environment: CheckState::Healthy,
            ai_usage: AiUsage::Unknown,
            order_prerequisite: CheckState::Healthy,
        }
    }
    fn resume_time() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(98765),
            Nanosecond::new(29),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn states<Problem: Copy>(
        problems: &[(Problem, SelfCheckAction)],
    ) -> Vec<(CheckState<Problem>, Option<SelfCheckAction>)> {
        let mut rows = vec![
            (CheckState::Healthy, None),
            (CheckState::Waiting(RetryDelay::Unknown), None),
            (
                CheckState::Waiting(RetryDelay::Known(
                    TimeSpan::try_new(SpanSeconds::new(37), Nanosecond::new(0)).expect("간격"),
                )),
                None,
            ),
            (
                CheckState::Bug(BugDiagnostic::Response(ResponseMismatch::Validation)),
                None,
            ),
            (CheckState::NoData(MissingData::Sector), None),
        ];
        rows.extend(
            problems
                .iter()
                .map(|(problem, action)| (CheckState::Problem(*problem), Some(*action))),
        );
        rows
    }
    fn key_rows() -> Vec<(SelfCheckInput, Option<SelfCheckAction>)> {
        let base = healthy();
        states(&[
            (
                TossKeyProblem::InvalidKey,
                SelfCheckAction::TossKey(TossKeyProblem::InvalidKey),
            ),
            (
                TossKeyProblem::Permissions,
                SelfCheckAction::TossKey(TossKeyProblem::Permissions),
            ),
            (
                TossKeyProblem::AccountNotFound,
                SelfCheckAction::TossKey(TossKeyProblem::AccountNotFound),
            ),
        ])
        .into_iter()
        .map(|(value, expected)| {
            (
                SelfCheckInput {
                    toss_key: value,
                    ..base
                },
                expected,
            )
        })
        .collect()
    }
    fn ip_rows() -> Vec<(SelfCheckInput, Option<SelfCheckAction>)> {
        let base = healthy();
        states(&[(IpProblem::NotAllowed, SelfCheckAction::RegisterIp)])
            .into_iter()
            .map(|(value, expected)| {
                (
                    SelfCheckInput {
                        allowed_ip: value,
                        ..base
                    },
                    expected,
                )
            })
            .collect()
    }
    fn duplicate_rows() -> Vec<(SelfCheckInput, Option<SelfCheckAction>)> {
        let base = healthy();
        states(&[(
            DuplicateKeyProblem::CompetingProgram,
            SelfCheckAction::StopOtherProgram,
        )])
        .into_iter()
        .map(|(value, expected)| {
            (
                SelfCheckInput {
                    duplicate_key_use: value,
                    ..base
                },
                expected,
            )
        })
        .collect()
    }
    fn install_rows() -> Vec<(SelfCheckInput, Option<SelfCheckAction>)> {
        let base = healthy();
        states(&[(
            ClaudeInstallProblem::NotInstalled,
            SelfCheckAction::InstallClaudeCode,
        )])
        .into_iter()
        .map(|(value, expected)| {
            (
                SelfCheckInput {
                    claude_install: value,
                    ..base
                },
                expected,
            )
        })
        .collect()
    }
    fn login_rows() -> Vec<(SelfCheckInput, Option<SelfCheckAction>)> {
        let base = healthy();
        states(&[(
            ClaudeLoginProblem::LoginRequired,
            SelfCheckAction::LoginClaudeCode,
        )])
        .into_iter()
        .map(|(value, expected)| {
            (
                SelfCheckInput {
                    claude_login: value,
                    ..base
                },
                expected,
            )
        })
        .collect()
    }
    fn environment_rows() -> Vec<(SelfCheckInput, Option<SelfCheckAction>)> {
        let base = healthy();
        let mut environment: Vec<_> = states::<AiEnvironmentProblem>(&[])
            .into_iter()
            .map(|(value, expected)| {
                (
                    SelfCheckInput {
                        ai_environment: value,
                        ..base
                    },
                    expected,
                )
            })
            .collect();
        environment.extend(
            [
                AiEnvironmentProblem::Tools,
                AiEnvironmentProblem::McpServers,
                AiEnvironmentProblem::Plugins,
                AiEnvironmentProblem::Skills,
                AiEnvironmentProblem::SlashCommands,
                AiEnvironmentProblem::PermissionMode,
                AiEnvironmentProblem::ApiKeySource,
                AiEnvironmentProblem::CliVersion,
            ]
            .into_iter()
            .map(|problem| {
                (
                    SelfCheckInput {
                        ai_environment: CheckState::Problem(problem),
                        ..base
                    },
                    None,
                )
            }),
        );
        environment
    }
    fn usage_rows() -> Vec<(SelfCheckInput, Option<SelfCheckAction>)> {
        let base = healthy();
        [
            AiUsage::Unknown,
            AiUsage::Available(application::Utilization::new(
                domain::Probability::try_new(
                    domain::Ratio::parse(&domain::NumericInput::try_new("0.37").expect("숫자"))
                        .expect("비율")
                        .as_decimal(),
                )
                .expect("확률"),
            )),
            AiUsage::Waiting(ResetTime::NotProvided),
            AiUsage::Waiting(ResetTime::Known(resume_time())),
        ]
        .into_iter()
        .map(|value| {
            (
                SelfCheckInput {
                    ai_usage: value,
                    ..base
                },
                None,
            )
        })
        .collect()
    }
    fn order_rows() -> Vec<(SelfCheckInput, Option<SelfCheckAction>)> {
        let base = healthy();
        states(&[
            (
                OrderPrerequisiteProblem::RiskNotice,
                SelfCheckAction::OrderPrerequisite(OrderPrerequisiteProblem::RiskNotice),
            ),
            (
                OrderPrerequisiteProblem::ExchangeIntegration,
                SelfCheckAction::OrderPrerequisite(OrderPrerequisiteProblem::ExchangeIntegration),
            ),
            (
                OrderPrerequisiteProblem::DollarBuyingPower,
                SelfCheckAction::OrderPrerequisite(OrderPrerequisiteProblem::DollarBuyingPower),
            ),
        ])
        .into_iter()
        .map(|(value, expected)| {
            (
                SelfCheckInput {
                    order_prerequisite: value,
                    ..base
                },
                expected,
            )
        })
        .collect()
    }
    fn fixtures() -> Vec<Vec<(SelfCheckInput, Option<SelfCheckAction>)>> {
        vec![
            key_rows(),
            ip_rows(),
            duplicate_rows(),
            install_rows(),
            login_rows(),
            environment_rows(),
            usage_rows(),
            order_rows(),
        ]
    }
    fn combine(rows: &[(SelfCheckInput, Option<SelfCheckAction>)]) -> SelfCheckInput {
        SelfCheckInput {
            toss_key: rows[0].0.toss_key,
            allowed_ip: rows[1].0.allowed_ip,
            duplicate_key_use: rows[2].0.duplicate_key_use,
            claude_install: rows[3].0.claude_install,
            claude_login: rows[4].0.claude_login,
            ai_environment: rows[5].0.ai_environment,
            ai_usage: rows[6].0.ai_usage,
            order_prerequisite: rows[7].0.order_prerequisite,
        }
    }
    fn news() -> Vec<SourceCheckRecord> {
        vec![
            SourceCheckRecord::OptionalCredential(CredentialSource::Dart),
            SourceCheckRecord::OptionalCredential(CredentialSource::Fred),
            SourceCheckRecord::Feed {
                source: FeedSource::Disclosure(domain::DisclosureSource::Dart),
                state: NewsCheckState::Failed(InformationError::InvalidCredential(
                    CredentialSource::Dart,
                )),
            },
            SourceCheckRecord::Feed {
                source: FeedSource::Disclosure(domain::DisclosureSource::Kind),
                state: NewsCheckState::Failed(InformationError::Unavailable(ResetTime::Known(
                    resume_time(),
                ))),
            },
        ]
    }
    #[test]
    fn design_table_and_all_slot_states_are_preserved() {
        for rows in fixtures() {
            for (input, expected) in rows {
                let result = evaluate_self_check(input, news());
                assert_eq!(result.banner, expected);
                assert_eq!(result.records, input);
                assert_eq!(result.sources, news());
            }
        }
    }
    fn verify_product(
        groups: &[Vec<(SelfCheckInput, Option<SelfCheckAction>)>],
        selected: &mut Vec<(SelfCheckInput, Option<SelfCheckAction>)>,
    ) {
        if let Some((head, tail)) = groups.split_first() {
            for row in head {
                selected.push(*row);
                verify_product(tail, selected);
                selected.pop();
            }
        } else {
            let input = combine(selected);
            let expected = selected.iter().find_map(|(_, action)| *action);
            let result = evaluate_self_check(input, Vec::new());
            assert_eq!(result.banner, expected);
            assert_eq!(result.records, input);
        }
    }
    #[test]
    fn every_combination_has_only_the_first_action() {
        verify_product(&fixtures(), &mut Vec::new());
    }
    proptest! {
        #![proptest_config(Config::with_cases(testkit::property_cases()))]
        #[test]
        fn arbitrary_combinations_preserve_hidden_records(indices in array::uniform8(0_usize..1000)) {
            let groups = fixtures();
            let selected: Vec<_> = groups.iter().zip(indices).map(|(group, index)| group[index.checked_rem(group.len()).expect("행 수")]).collect();
            let input = combine(&selected);
            let result = evaluate_self_check(input, news());
            prop_assert_eq!(result.banner, selected.iter().find_map(|(_, action)| *action));
            prop_assert_eq!(result.records, input);
            prop_assert_eq!(result.sources, news());
        }
    }
    #[test]
    fn event_compares_exactly_slots_four_through_eight_even_with_broker_changes() {
        let old = evaluate_self_check(healthy(), Vec::new());
        assert_eq!(self_check_change(&old, &old), SelfCheckChange::Unchanged);
        for (slot, rows) in fixtures().into_iter().enumerate() {
            for (input, _) in rows.into_iter().skip(1) {
                let next = evaluate_self_check(input, news());
                let expected = [
                    SelfCheckChange::Unchanged,
                    SelfCheckChange::Unchanged,
                    SelfCheckChange::Unchanged,
                    SelfCheckChange::Publish,
                    SelfCheckChange::Publish,
                    SelfCheckChange::Publish,
                    SelfCheckChange::Publish,
                    SelfCheckChange::Publish,
                ][slot];
                assert_eq!(self_check_change(&old, &next), expected);
                let concurrent = SelfCheckInput {
                    toss_key: CheckState::Problem(TossKeyProblem::InvalidKey),
                    allowed_ip: CheckState::Problem(IpProblem::NotAllowed),
                    duplicate_key_use: CheckState::Problem(DuplicateKeyProblem::CompetingProgram),
                    ..input
                };
                assert_eq!(
                    self_check_change(&old, &evaluate_self_check(concurrent, news())),
                    expected
                );
                assert_eq!(self_check_change(&next, &old), expected);
            }
        }
        assert_eq!(
            self_check_change(&old, &evaluate_self_check(healthy(), news())),
            SelfCheckChange::Unchanged
        );
    }
    fn require_user_action_inventory(action: UserAction) {
        match action {
            UserAction::CheckCredentials
            | UserAction::CheckPermissions
            | UserAction::CheckAccount
            | UserAction::CheckIpAllowlist
            | UserAction::StopCompetingKeyUse => (),
        }
    }
    fn require_error_inventory(class: ErrorClass) {
        match class {
            ErrorClass::UserAction(action) => require_user_action_inventory(action),
            ErrorClass::Wait(_) | ErrorClass::Bug(_) | ErrorClass::NoData(_) => (),
        }
    }
    #[test]
    fn a05_conversion_matches_fixed_table() {
        let cases = [
            (
                ErrorClass::UserAction(UserAction::CheckCredentials),
                TossCheckObservation::Key(CheckState::Problem(TossKeyProblem::InvalidKey)),
            ),
            (
                ErrorClass::UserAction(UserAction::CheckPermissions),
                TossCheckObservation::Key(CheckState::Problem(TossKeyProblem::Permissions)),
            ),
            (
                ErrorClass::UserAction(UserAction::CheckAccount),
                TossCheckObservation::Key(CheckState::Problem(TossKeyProblem::AccountNotFound)),
            ),
            (
                ErrorClass::UserAction(UserAction::CheckIpAllowlist),
                TossCheckObservation::Ip(CheckState::Problem(IpProblem::NotAllowed)),
            ),
            (
                ErrorClass::UserAction(UserAction::StopCompetingKeyUse),
                TossCheckObservation::DuplicateKey(CheckState::Problem(
                    DuplicateKeyProblem::CompetingProgram,
                )),
            ),
            (
                ErrorClass::Wait(RetryDelay::Unknown),
                TossCheckObservation::Key(CheckState::Waiting(RetryDelay::Unknown)),
            ),
            (
                ErrorClass::Wait(RetryDelay::Known(
                    TimeSpan::try_new(SpanSeconds::new(19), Nanosecond::new(0)).expect("간격"),
                )),
                TossCheckObservation::Key(CheckState::Waiting(RetryDelay::Known(
                    TimeSpan::try_new(SpanSeconds::new(19), Nanosecond::new(0)).expect("간격"),
                ))),
            ),
            (
                ErrorClass::Bug(BugDiagnostic::Response(ResponseMismatch::Schema)),
                TossCheckObservation::Key(CheckState::Bug(BugDiagnostic::Response(
                    ResponseMismatch::Schema,
                ))),
            ),
            (
                ErrorClass::NoData(MissingData::ExchangeRate),
                TossCheckObservation::Key(CheckState::NoData(MissingData::ExchangeRate)),
            ),
        ];
        for (class, expected) in cases {
            require_error_inventory(class);
            assert_eq!(toss_check_observation(class), expected);
        }
    }
    fn require_isolation_inventory(failure: IsolationFailure) {
        match failure {
            IsolationFailure::NotInstalled
            | IsolationFailure::LoginRequired
            | IsolationFailure::Tools
            | IsolationFailure::McpServers
            | IsolationFailure::Plugins
            | IsolationFailure::Skills
            | IsolationFailure::SlashCommands
            | IsolationFailure::PermissionMode
            | IsolationFailure::ApiKeySource
            | IsolationFailure::CliVersion => (),
        }
    }
    fn isolation_cases() -> [(
        IsolationFailure,
        AiCheckObservation,
        Option<SelfCheckAction>,
    ); 10] {
        [
            (
                IsolationFailure::NotInstalled,
                AiCheckObservation::Install(ClaudeInstallProblem::NotInstalled),
                Some(SelfCheckAction::InstallClaudeCode),
            ),
            (
                IsolationFailure::LoginRequired,
                AiCheckObservation::Login(ClaudeLoginProblem::LoginRequired),
                Some(SelfCheckAction::LoginClaudeCode),
            ),
            (
                IsolationFailure::Tools,
                AiCheckObservation::Environment(AiEnvironmentProblem::Tools),
                None,
            ),
            (
                IsolationFailure::McpServers,
                AiCheckObservation::Environment(AiEnvironmentProblem::McpServers),
                None,
            ),
            (
                IsolationFailure::Plugins,
                AiCheckObservation::Environment(AiEnvironmentProblem::Plugins),
                None,
            ),
            (
                IsolationFailure::Skills,
                AiCheckObservation::Environment(AiEnvironmentProblem::Skills),
                None,
            ),
            (
                IsolationFailure::SlashCommands,
                AiCheckObservation::Environment(AiEnvironmentProblem::SlashCommands),
                None,
            ),
            (
                IsolationFailure::PermissionMode,
                AiCheckObservation::Environment(AiEnvironmentProblem::PermissionMode),
                None,
            ),
            (
                IsolationFailure::ApiKeySource,
                AiCheckObservation::Environment(AiEnvironmentProblem::ApiKeySource),
                None,
            ),
            (
                IsolationFailure::CliVersion,
                AiCheckObservation::Environment(AiEnvironmentProblem::CliVersion),
                None,
            ),
        ]
    }
    #[test]
    fn isolation_conversion_routes_install_login_and_environment() {
        for (failure, expected, banner) in isolation_cases() {
            require_isolation_inventory(failure);
            let converted = isolation_check_observation(failure);
            assert_eq!(converted, expected);
            let mut input = healthy();
            match converted {
                AiCheckObservation::Install(problem) => {
                    input.claude_install = CheckState::Problem(problem);
                }
                AiCheckObservation::Login(problem) => {
                    input.claude_login = CheckState::Problem(problem);
                }
                AiCheckObservation::Environment(problem) => {
                    input.ai_environment = CheckState::Problem(problem);
                }
            }
            assert_eq!(evaluate_self_check(input, Vec::new()).banner, banner);
        }
    }
}
