use crate::{MarketTradingStatus, OrderPermission};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalAvailability {
    Available,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderButtonHiddenReason {
    OrdersDisabled,
    MarketNotOpen,
    NoProposal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderButtonState {
    Shown,
    Hidden(OrderButtonHiddenReason),
}

#[must_use]
pub const fn order_button_state(
    permission: OrderPermission,
    market: MarketTradingStatus,
    proposal: ProposalAvailability,
) -> OrderButtonState {
    match (permission, market, proposal) {
        (OrderPermission::Disabled, _, _) => {
            OrderButtonState::Hidden(OrderButtonHiddenReason::OrdersDisabled)
        }
        (
            OrderPermission::Enabled,
            MarketTradingStatus::Closed | MarketTradingStatus::Unknown,
            _,
        ) => OrderButtonState::Hidden(OrderButtonHiddenReason::MarketNotOpen),
        (OrderPermission::Enabled, MarketTradingStatus::Open, ProposalAvailability::Missing) => {
            OrderButtonState::Hidden(OrderButtonHiddenReason::NoProposal)
        }
        (OrderPermission::Enabled, MarketTradingStatus::Open, ProposalAvailability::Available) => {
            OrderButtonState::Shown
        }
    }
}

#[must_use]
pub fn saved_order_permission(saved: Option<crate::RiskSettings>) -> OrderPermission {
    saved.map_or(OrderPermission::Disabled, crate::RiskSettings::orders)
}

#[cfg(test)]
mod tests {
    use super::{
        MarketTradingStatus, OrderButtonHiddenReason, OrderButtonState, OrderPermission,
        ProposalAvailability, order_button_state, saved_order_permission,
    };

    const PERMISSIONS: [OrderPermission; 2] = [OrderPermission::Disabled, OrderPermission::Enabled];
    const MARKETS: [MarketTradingStatus; 3] = [
        MarketTradingStatus::Open,
        MarketTradingStatus::Closed,
        MarketTradingStatus::Unknown,
    ];
    const PROPOSALS: [ProposalAvailability; 2] = [
        ProposalAvailability::Available,
        ProposalAvailability::Missing,
    ];

    #[test]
    fn button_is_shown_only_when_enabled_open_and_proposal_present() {
        let cases = PERMISSIONS.iter().flat_map(|permission| {
            MARKETS.iter().flat_map(move |market| {
                PROPOSALS
                    .iter()
                    .map(move |proposal| (*permission, *market, *proposal))
            })
        });
        for (permission, market, proposal) in cases {
            let state = order_button_state(permission, market, proposal);
            let expected_shown = permission == OrderPermission::Enabled
                && market == MarketTradingStatus::Open
                && proposal == ProposalAvailability::Available;
            assert_eq!(state == OrderButtonState::Shown, expected_shown);
        }
    }

    #[test]
    fn disabled_switch_hides_button_before_any_other_reason() {
        assert_eq!(
            order_button_state(
                OrderPermission::Disabled,
                MarketTradingStatus::Closed,
                ProposalAvailability::Missing,
            ),
            OrderButtonState::Hidden(OrderButtonHiddenReason::OrdersDisabled)
        );
        assert_eq!(
            order_button_state(
                OrderPermission::Enabled,
                MarketTradingStatus::Unknown,
                ProposalAvailability::Available,
            ),
            OrderButtonState::Hidden(OrderButtonHiddenReason::MarketNotOpen)
        );
        assert_eq!(
            order_button_state(
                OrderPermission::Enabled,
                MarketTradingStatus::Open,
                ProposalAvailability::Missing,
            ),
            OrderButtonState::Hidden(OrderButtonHiddenReason::NoProposal)
        );
    }

    #[test]
    fn fresh_install_without_saved_settings_is_disabled() {
        assert_eq!(saved_order_permission(None), OrderPermission::Disabled);
    }
}
