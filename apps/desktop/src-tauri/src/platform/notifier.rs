use application::{
    EventSubscriber, Notification, Notifier, RetryDelay, SubscriberFailure, SystemFailure,
    SystemPort, SystemPortError, WriteCompleted,
};
use domain::{DisclosureKey, DomainEvent, DomainEventKind};
use std::collections::BTreeSet;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationToggle {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotificationPreferences {
    pub new_signal: NotificationToggle,
    pub urgent_disclosure: NotificationToggle,
    pub briefing_finished: NotificationToggle,
}

impl Default for NotificationPreferences {
    fn default() -> Self {
        Self {
            new_signal: NotificationToggle::Enabled,
            urgent_disclosure: NotificationToggle::Enabled,
            briefing_finished: NotificationToggle::Enabled,
        }
    }
}

impl NotificationPreferences {
    #[must_use]
    pub const fn state_for(self, notification: Notification) -> NotificationToggle {
        match notification {
            Notification::NewSignal => self.new_signal,
            Notification::UrgentDisclosure => self.urgent_disclosure,
            Notification::BriefingFinished => self.briefing_finished,
        }
    }

    pub const fn set(&mut self, notification: Notification, toggle: NotificationToggle) {
        match notification {
            Notification::NewSignal => self.new_signal = toggle,
            Notification::UrgentDisclosure => self.urgent_disclosure = toggle,
            Notification::BriefingFinished => self.briefing_finished = toggle,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotificationMessage {
    pub title: &'static str,
    pub body: &'static str,
}

#[must_use]
pub const fn message_for(notification: Notification) -> NotificationMessage {
    match notification {
        Notification::NewSignal => NotificationMessage {
            title: "새 매매 신호",
            body: "확인할 수 있는 새 신호가 도착했습니다.",
        },
        Notification::UrgentDisclosure => NotificationMessage {
            title: "보유 종목 긴급 공시",
            body: "보유 종목에 긴급 공시가 올라왔습니다.",
        },
        Notification::BriefingFinished => NotificationMessage {
            title: "아침 브리핑 준비 완료",
            body: "오늘 브리핑이 준비되었습니다.",
        },
    }
}

pub trait NotificationDelivery: Send + Sync {
    fn deliver(
        &self,
        notification: Notification,
        message: NotificationMessage,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct UnavailableNotificationDelivery;

impl NotificationDelivery for UnavailableNotificationDelivery {
    fn deliver(
        &self,
        _notification: Notification,
        _message: NotificationMessage,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        std::future::ready(Err(SystemPortError::new(
            SystemPort::Notifier,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        )))
    }
}

#[derive(Debug)]
pub struct FilteredNotifier<D> {
    preferences: RwLock<NotificationPreferences>,
    delivery: D,
}

impl<D> FilteredNotifier<D> {
    #[must_use]
    pub const fn new(delivery: D, preferences: NotificationPreferences) -> Self {
        Self {
            preferences: RwLock::new(preferences),
            delivery,
        }
    }

    #[must_use]
    pub fn preferences(&self) -> NotificationPreferences {
        if let Ok(guard) = self.preferences.read() {
            *guard
        } else {
            NotificationPreferences::default()
        }
    }

    pub fn set_toggle(
        &self,
        notification: Notification,
        toggle: NotificationToggle,
    ) -> Result<(), SystemPortError> {
        let mut guard = self.preferences.write().map_err(|_error| {
            SystemPortError::new(
                SystemPort::Notifier,
                SystemFailure::Unavailable(RetryDelay::Unknown),
            )
        })?;
        guard.set(notification, toggle);
        Ok(())
    }

    #[must_use]
    pub const fn delivery(&self) -> &D {
        &self.delivery
    }
}

impl<D: NotificationDelivery> Notifier for FilteredNotifier<D> {
    async fn notify(&self, notification: Notification) -> Result<WriteCompleted, SystemPortError> {
        let toggle = self.preferences().state_for(notification);
        match toggle {
            NotificationToggle::Disabled => Ok(WriteCompleted),
            NotificationToggle::Enabled => {
                self.delivery
                    .deliver(notification, message_for(notification))
                    .await
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisclosureUrgency {
    Urgent,
    NonUrgent,
}

pub trait DisclosureUrgencyLookup: Send + Sync {
    fn urgency(&self, key: &DisclosureKey) -> DisclosureUrgency;
}

pub trait NotificationDispatch: Send + Sync {
    fn dispatch(&self, notification: Notification) -> Result<(), SubscriberFailure>;
}

#[derive(Debug)]
pub struct AsyncRuntimeNotificationDispatch<N> {
    notifier: Arc<N>,
}

impl<N> AsyncRuntimeNotificationDispatch<N> {
    #[must_use]
    pub const fn new(notifier: Arc<N>) -> Self {
        Self { notifier }
    }
}

impl<N: Notifier + 'static> NotificationDispatch for AsyncRuntimeNotificationDispatch<N> {
    fn dispatch(&self, notification: Notification) -> Result<(), SubscriberFailure> {
        let target = Arc::clone(&self.notifier);
        let _task = tauri::async_runtime::spawn(async move {
            let _result = target.notify(notification).await;
        });
        Ok(())
    }
}

#[derive(Debug)]
pub struct NotificationEventSubscriber<D, L> {
    dispatcher: D,
    urgency_lookup: L,
}

impl<D, L> NotificationEventSubscriber<D, L> {
    #[must_use]
    pub const fn new(dispatcher: D, urgency_lookup: L) -> Self {
        Self {
            dispatcher,
            urgency_lookup,
        }
    }

    #[must_use]
    pub fn subscribed_kinds() -> BTreeSet<DomainEventKind> {
        BTreeSet::from([
            DomainEventKind::SignalRaised,
            DomainEventKind::DisclosureTriggerRecorded,
        ])
    }
}

impl<D: NotificationDispatch, L: DisclosureUrgencyLookup> EventSubscriber
    for NotificationEventSubscriber<D, L>
{
    fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
        match event {
            DomainEvent::SignalRaised(_) => self.dispatcher.dispatch(Notification::NewSignal),
            DomainEvent::DisclosureTriggerRecorded(key) => match self.urgency_lookup.urgency(key) {
                DisclosureUrgency::Urgent => {
                    self.dispatcher.dispatch(Notification::UrgentDisclosure)
                }
                DisclosureUrgency::NonUrgent => Ok(()),
            },
            DomainEvent::PortfolioSyncRecorded(_)
            | DomainEvent::TransactionsSyncRecorded(_)
            | DomainEvent::PriceUpdated(_)
            | DomainEvent::PriceMoved(_)
            | DomainEvent::DisclosureArrived(_)
            | DomainEvent::NewsActivityIncreased(_)
            | DomainEvent::ProposalCreated(_)
            | DomainEvent::ProposalExpired(_)
            | DomainEvent::ProposalApproved(_)
            | DomainEvent::ProposalRejected(_)
            | DomainEvent::ProposalSkipped(_)
            | DomainEvent::OrderSubmitted(_)
            | DomainEvent::OrderOutcomeUnknown(_)
            | DomainEvent::BrokerConnectionChanged
            | DomainEvent::SettingsChanged
            | DomainEvent::PolicyChanged
            | DomainEvent::OrderSwitchChanged
            | DomainEvent::WatchlistChanged
            | DomainEvent::ThesisUpdated(_)
            | DomainEvent::SelfCheckChanged
            | DomainEvent::MarketSessionChanged(_)
            | DomainEvent::SignalCleared(_)
            | DomainEvent::EstimateSubmitted(_)
            | DomainEvent::EstimateResolved(_)
            | DomainEvent::ValuationSnapshotTaken(_)
            | DomainEvent::OrderRejected(_)
            | DomainEvent::OrderOutcomeResolved(_)
            | DomainEvent::OrderFilled(_)
            | DomainEvent::OrderCancelled(_)
            | DomainEvent::OrderModified(_)
            | DomainEvent::CollectionFinished(_)
            | DomainEvent::NewsClusterGrew(_)
            | DomainEvent::AiBudgetChanged
            | DomainEvent::AccountSelectionChanged
            | DomainEvent::StoredDataChanged
            | DomainEvent::FxRateUpdated(_) => Ok(()),
        }
    }
}
