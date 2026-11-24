use application::{EventSubscriber, SubscriberFailure};
use domain::DomainEvent;
use tauri::{AppHandle, Runtime};
use tauri_specta::Event;

use super::event::UiEvent;

#[derive(Debug, Clone)]
pub struct UiEventBridge<R: Runtime> {
    handle: AppHandle<R>,
}

impl<R: Runtime> UiEventBridge<R> {
    #[must_use]
    pub const fn new(handle: AppHandle<R>) -> Self {
        Self { handle }
    }

    pub fn emit_domain_event(&self, event: DomainEvent) -> Result<(), SubscriberFailure> {
        UiEvent::from(event)
            .emit(&self.handle)
            .map_err(|_error| SubscriberFailure::Unavailable)
    }
}

impl<R: Runtime> EventSubscriber for UiEventBridge<R> {
    fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
        self.emit_domain_event(event.clone())
    }
}
