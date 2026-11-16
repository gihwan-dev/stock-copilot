use crate::{DatabasePath, connection, failure};
use application::{SecretValue, SystemPortError};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use std::sync::mpsc::{self, Sender};
use tokio::sync::oneshot;

type Job = Box<dyn FnOnce(&mut Connection) + Send>;

enum Request {
    Run(Job),
    Stop,
}

pub(crate) struct Writer {
    requests: Sender<Request>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Writer {
    pub(crate) fn open_migrated(
        path: &DatabasePath,
        key: &SecretValue,
        at: domain::Timestamp,
    ) -> Result<Self, SystemPortError> {
        Self::start(path, key, move |connection| {
            crate::migration::run(connection, at)
        })
    }

    fn start(
        path: &DatabasePath,
        key: &SecretValue,
        initialize: impl FnOnce(&mut Connection) -> Result<(), SystemPortError> + Send + 'static,
    ) -> Result<Self, SystemPortError> {
        let mut connection = connection::open(path, key, connection::Access::Write)?;
        let (ready, initialized) = mpsc::sync_channel(1);
        let (requests, receiver) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name(String::from("encrypted-store-writer"))
            .spawn(move || {
                let result = initialize(&mut connection);
                let failed = result.is_err();
                if ready.send(result).is_err() || failed {
                    return;
                }
                while let Ok(Request::Run(job)) = receiver.recv() {
                    job(&mut connection);
                }
            })
            .map_err(|error| failure::io(&error))?;
        let writer = Self {
            requests,
            thread: Some(thread),
        };
        initialized
            .recv()
            .map_err(|_error| failure::integrity())??;
        Ok(writer)
    }

    pub(crate) fn close(mut self) -> Result<application::WriteCompleted, SystemPortError> {
        let sent = self
            .requests
            .send(Request::Stop)
            .map_err(|_error| failure::integrity());
        if let Some(thread) = self.thread.take() {
            thread.join().map_err(|_error| failure::integrity())?;
        }
        sent?;
        Ok(application::WriteCompleted)
    }

    pub(crate) async fn execute<T, F>(&self, operation: F) -> Result<T, SystemPortError>
    where
        T: Send + 'static,
        F: FnOnce(&Transaction<'_>) -> rusqlite::Result<T> + Send + 'static,
    {
        self.submit(operation)?
            .await
            .map_err(|_error| failure::integrity())?
    }

    pub(crate) fn submit<T, F>(
        &self,
        operation: F,
    ) -> Result<oneshot::Receiver<Result<T, SystemPortError>>, SystemPortError>
    where
        T: Send + 'static,
        F: FnOnce(&Transaction<'_>) -> rusqlite::Result<T> + Send + 'static,
    {
        let (sender, receiver) = oneshot::channel();
        let job = Box::new(move |connection: &mut Connection| {
            let result = (|| {
                let transaction =
                    connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let value = operation(&transaction)?;
                transaction.commit()?;
                Ok(value)
            })()
            .map_err(|error| failure::sqlite(&error));
            match sender.send(result) {
                Ok(()) | Err(_) => {}
            }
        });
        self.requests
            .send(Request::Run(job))
            .map_err(|_error| failure::integrity())?;
        Ok(receiver)
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        match self.requests.send(Request::Stop) {
            Ok(()) | Err(_) => {}
        }
        if let Some(thread) = self.thread.take() {
            match thread.join() {
                Ok(()) | Err(_) => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Request, Writer};
    use crate::DatabasePath;
    use application::SecretValue;

    #[test]
    fn dropping_writer_disconnects_retained_request_endpoint() {
        let fixture = crate::tests::Fixture::new();
        let endpoint;
        {
            let writer = Writer::open_migrated(
                &DatabasePath::new(fixture.path()),
                &SecretValue::new(vec![23; 32]),
                crate::tests::timestamp(),
            )
            .expect("writer");
            endpoint = writer.requests.clone();
        };
        endpoint
            .send(Request::Stop)
            .expect_err("writer has exited before drop returns");
    }
}
