#[cfg(test)]
mod tests {
    use application::{
        BugDiagnostic, CredentialSource, ErrorClass, FeedSource, InformationError,
        IsolationFailure, MissingData, NewsCheckRecord, NewsCheckState, ResetTime,
        ResponseMismatch, RetryDelay, SelfCheckAction, SelfCheckChange, SelfCheckChangeCause,
        SelfCheckDifference, SelfCheckKind, SelfCheckObservation, UserAction, evaluate_self_check,
        self_check_change, self_check_difference,
    };
    use domain::Timestamp;
    use proptest::{array, prop_assert_eq, proptest, test_runner::Config as ProptestConfig};

    const CHECKS: [SelfCheckKind; 8] = [
        SelfCheckKind::TossKey,
        SelfCheckKind::AllowedIp,
        SelfCheckKind::DuplicateKeyUse,
        SelfCheckKind::ClaudeCodeInstallation,
        SelfCheckKind::ClaudeCodeLogin,
        SelfCheckKind::AiEnvironment,
        SelfCheckKind::AiUsageLimit,
        SelfCheckKind::OrderPrerequisites,
    ];

    fn action(kind: SelfCheckKind) -> Option<SelfCheckAction> {
        match kind {
            SelfCheckKind::TossKey => Some(SelfCheckAction::ReplaceTossKey),
            SelfCheckKind::AllowedIp => Some(SelfCheckAction::RegisterIp),
            SelfCheckKind::DuplicateKeyUse => Some(SelfCheckAction::StopOtherProgram),
            SelfCheckKind::ClaudeCodeInstallation => Some(SelfCheckAction::InstallClaudeCode),
            SelfCheckKind::ClaudeCodeLogin => Some(SelfCheckAction::LoginClaudeCode),
            SelfCheckKind::AiEnvironment | SelfCheckKind::AiUsageLimit => None,
            SelfCheckKind::OrderPrerequisites => Some(SelfCheckAction::ReviewRiskNotice),
        }
    }

    fn expected(kind: SelfCheckKind, state: SelfCheckObservation) -> Option<SelfCheckAction> {
        match state {
            SelfCheckObservation::Healthy
            | SelfCheckObservation::Waiting(_)
            | SelfCheckObservation::EnvironmentRejected(_)
            | SelfCheckObservation::OptionalCredential(_) => None,
            SelfCheckObservation::NeedsAction(value) => match value {
                SelfCheckAction::ReplaceTossKey => {
                    (kind == SelfCheckKind::TossKey).then_some(value)
                }
                SelfCheckAction::RegisterIp => (kind == SelfCheckKind::AllowedIp).then_some(value),
                SelfCheckAction::StopOtherProgram => {
                    (kind == SelfCheckKind::DuplicateKeyUse).then_some(value)
                }
                SelfCheckAction::InstallClaudeCode => {
                    (kind == SelfCheckKind::ClaudeCodeInstallation).then_some(value)
                }
                SelfCheckAction::LoginClaudeCode => {
                    (kind == SelfCheckKind::ClaudeCodeLogin).then_some(value)
                }
                SelfCheckAction::ReviewRiskNotice
                | SelfCheckAction::IntegrateExchange
                | SelfCheckAction::ProvideDollarBuyingPower => {
                    (kind == SelfCheckKind::OrderPrerequisites).then_some(value)
                }
            },
            SelfCheckObservation::Classified(class) => {
                match class {
                    ErrorClass::Wait(_) | ErrorClass::Bug(_) | ErrorClass::NoData(_) => None,
                    ErrorClass::UserAction(value) => match value {
                        UserAction::CheckCredentials => (kind == SelfCheckKind::TossKey)
                            .then_some(SelfCheckAction::ReplaceTossKey),
                        UserAction::CheckIpAllowlist => (kind == SelfCheckKind::AllowedIp)
                            .then_some(SelfCheckAction::RegisterIp),
                        UserAction::StopCompetingKeyUse => (kind == SelfCheckKind::DuplicateKeyUse)
                            .then_some(SelfCheckAction::StopOtherProgram),
                        UserAction::CheckAccount | UserAction::CheckPermissions => None,
                    },
                }
            }
        }
    }

    fn states() -> Vec<SelfCheckObservation> {
        let mut values = vec![
            SelfCheckObservation::Healthy,
            SelfCheckObservation::Waiting(ResetTime::NotProvided),
            SelfCheckObservation::Waiting(ResetTime::Known(
                Timestamp::from_parts(
                    domain::UnixSeconds::new(12345),
                    domain::Nanosecond::new(17),
                    domain::OffsetSeconds::new(0),
                )
                .expect("시각"),
            )),
            SelfCheckObservation::EnvironmentRejected(IsolationFailure::Tools),
            SelfCheckObservation::OptionalCredential(CredentialSource::Dart),
            SelfCheckObservation::OptionalCredential(CredentialSource::Fred),
            SelfCheckObservation::Classified(ErrorClass::Wait(RetryDelay::Unknown)),
            SelfCheckObservation::Classified(ErrorClass::Bug(BugDiagnostic::Response(
                ResponseMismatch::Schema,
            ))),
            SelfCheckObservation::Classified(ErrorClass::NoData(MissingData::Stock)),
        ];
        for value in [
            UserAction::CheckCredentials,
            UserAction::CheckIpAllowlist,
            UserAction::StopCompetingKeyUse,
            UserAction::CheckAccount,
            UserAction::CheckPermissions,
        ] {
            values.push(SelfCheckObservation::Classified(ErrorClass::UserAction(
                value,
            )));
        }
        for value in [
            SelfCheckAction::ReplaceTossKey,
            SelfCheckAction::RegisterIp,
            SelfCheckAction::StopOtherProgram,
            SelfCheckAction::InstallClaudeCode,
            SelfCheckAction::LoginClaudeCode,
            SelfCheckAction::ReviewRiskNotice,
            SelfCheckAction::IntegrateExchange,
            SelfCheckAction::ProvideDollarBuyingPower,
        ] {
            values.push(SelfCheckObservation::NeedsAction(value));
        }
        values
    }

    #[test]
    fn self_check_decision_table_and_order() {
        for (index, kind) in CHECKS.into_iter().enumerate() {
            for state in states() {
                let mut input = [SelfCheckObservation::Healthy; 8];
                input[index] = state;
                let result = evaluate_self_check(input, Vec::new());
                assert_eq!(
                    result
                        .banner
                        .map(|value| (value.check, value.action, value.resumes_at)),
                    expected(kind, state).map(|value| (kind, value, ResetTime::NotProvided))
                );
                assert_eq!(result.records.map(|record| record.check), CHECKS);
                assert_eq!(result.records.map(|record| record.observation), input);
            }
        }
    }

    #[test]
    fn self_check_masked_records_and_news_are_preserved() {
        let input = [
            SelfCheckObservation::NeedsAction(SelfCheckAction::ReplaceTossKey),
            SelfCheckObservation::NeedsAction(SelfCheckAction::RegisterIp),
            SelfCheckObservation::NeedsAction(SelfCheckAction::StopOtherProgram),
            SelfCheckObservation::NeedsAction(SelfCheckAction::InstallClaudeCode),
            SelfCheckObservation::NeedsAction(SelfCheckAction::LoginClaudeCode),
            SelfCheckObservation::EnvironmentRejected(IsolationFailure::Plugins),
            SelfCheckObservation::Waiting(ResetTime::Known(
                Timestamp::from_parts(
                    domain::UnixSeconds::new(98765),
                    domain::Nanosecond::new(29),
                    domain::OffsetSeconds::new(0),
                )
                .expect("시각"),
            )),
            SelfCheckObservation::NeedsAction(SelfCheckAction::IntegrateExchange),
        ];
        let news = vec![NewsCheckRecord {
            source: FeedSource::Disclosure(domain::DisclosureSource::Dart),
            state: NewsCheckState::Failed(InformationError::InvalidCredential(
                CredentialSource::Dart,
            )),
        }];
        let result = evaluate_self_check(input, news.clone());
        assert_eq!(result.news, news);
        assert_eq!(result.records.map(|record| record.observation), input);
        assert_eq!(
            result.banner.map(|banner| banner.check),
            Some(SelfCheckKind::TossKey)
        );
    }

    #[test]
    fn self_check_change_compares_hidden_status_and_suppresses_broker_events() {
        let input = [SelfCheckObservation::Healthy; 8];
        let old = evaluate_self_check(input, Vec::new());
        for cause in [
            SelfCheckChangeCause::BrokerConnection,
            SelfCheckChangeCause::OtherCheck,
        ] {
            assert_eq!(self_check_difference(&old, &old), SelfCheckDifference::Same);
            assert_eq!(
                self_check_change(&old, &old, cause),
                SelfCheckChange::Unchanged
            );
            for index in 0..8 {
                let mut changed = input;
                changed[index] = SelfCheckObservation::Waiting(ResetTime::NotProvided);
                let new = evaluate_self_check(changed, Vec::new());
                assert_eq!(
                    self_check_difference(&old, &new),
                    SelfCheckDifference::Changed
                );
                assert_eq!(
                    self_check_change(&old, &new, cause),
                    if cause == SelfCheckChangeCause::OtherCheck && index >= 3 {
                        SelfCheckChange::Publish
                    } else {
                        SelfCheckChange::Unchanged
                    }
                );
            }
            let new = evaluate_self_check(
                input,
                vec![NewsCheckRecord {
                    source: FeedSource::Disclosure(domain::DisclosureSource::Dart),
                    state: NewsCheckState::NoData,
                }],
            );
            assert_eq!(
                self_check_change(&old, &new, cause),
                SelfCheckChange::Unchanged
            );
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4096))]
        #[test]
        fn self_check_combinations_match_independent_priority(indices in array::uniform8(0_usize..22)) {
            let choices = states();
            let input = indices.map(|index| choices[index]);
            let news = vec![NewsCheckRecord { source: FeedSource::Disclosure(domain::DisclosureSource::Kind), state: NewsCheckState::Failed(InformationError::InvalidCredential(CredentialSource::Fred)) }];
            let result = evaluate_self_check(input, news);
            let wanted = CHECKS.into_iter().zip(input).find_map(|(kind, state)| expected(kind, state).map(|action| (kind,action)));
            prop_assert_eq!(result.banner.map(|banner| (banner.check,banner.action)), wanted);
            prop_assert_eq!(result.records.map(|record| record.observation), input);
            prop_assert_eq!(result.records.map(|record| record.check), CHECKS);
        }
    }

    fn subset_state(mask: u16, index: usize) -> SelfCheckObservation {
        if mask & (1 << index) == 0 {
            SelfCheckObservation::Healthy
        } else {
            action(CHECKS[index]).map_or(
                SelfCheckObservation::Waiting(ResetTime::NotProvided),
                SelfCheckObservation::NeedsAction,
            )
        }
    }

    #[test]
    fn self_check_every_actionable_subset_picks_first() {
        for mask in 0_u16..256 {
            let input = std::array::from_fn(|index| subset_state(mask, index));
            let result = evaluate_self_check(input, Vec::new());
            let wanted = CHECKS.iter().enumerate().find_map(|(index, kind)| {
                (mask & (1 << index) != 0)
                    .then(|| action(*kind).map(|action| (*kind, action)))
                    .flatten()
            });
            assert_eq!(
                result.banner.map(|banner| (banner.check, banner.action)),
                wanted
            );
        }
    }
}
