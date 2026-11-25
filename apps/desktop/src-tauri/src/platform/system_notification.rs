use crate::platform::notifier::{NotificationDelivery, NotificationMessage};
use application::{
    Notification, RetryDelay, SystemFailure, SystemPort, SystemPortError, WriteCompleted,
};
use std::sync::{Arc, OnceLock};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_notification::NotificationExt;

#[derive(Debug)]
pub struct NotificationHandleSlot<R: Runtime>(OnceLock<AppHandle<R>>);

impl<R: Runtime> Default for NotificationHandleSlot<R> {
    fn default() -> Self {
        Self(OnceLock::new())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationBinding {
    Bound,
    AlreadyBound,
}

impl<R: Runtime> NotificationHandleSlot<R> {
    pub fn bind(&self, handle: AppHandle<R>) -> NotificationBinding {
        match self.0.set(handle) {
            Ok(()) => NotificationBinding::Bound,
            Err(_existing) => NotificationBinding::AlreadyBound,
        }
    }
}

#[derive(Debug)]
pub struct SystemNotificationDelivery<R: Runtime> {
    slot: Arc<NotificationHandleSlot<R>>,
}

impl<R: Runtime> SystemNotificationDelivery<R> {
    #[must_use]
    pub const fn new(slot: Arc<NotificationHandleSlot<R>>) -> Self {
        Self { slot }
    }

    fn show(&self, message: NotificationMessage) -> Result<WriteCompleted, SystemPortError> {
        let unavailable = || {
            SystemPortError::new(
                SystemPort::Notifier,
                SystemFailure::Unavailable(RetryDelay::Unknown),
            )
        };
        let handle = self.slot.0.get().ok_or_else(unavailable)?;
        if handle
            .try_state::<tauri_plugin_notification::Notification<R>>()
            .is_none()
        {
            return Err(unavailable());
        }
        handle
            .notification()
            .builder()
            .title(message.title)
            .body(message.body)
            .show()
            .map(|()| WriteCompleted)
            .map_err(|_error| unavailable())
    }
}

impl<R: Runtime> NotificationDelivery for SystemNotificationDelivery<R> {
    fn deliver(
        &self,
        _notification: Notification,
        message: NotificationMessage,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        std::future::ready(self.show(message))
    }
}
