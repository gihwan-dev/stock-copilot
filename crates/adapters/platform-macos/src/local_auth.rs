use crate::order_presence::{
    AuthenticationReason, DeviceOwnerAuthBackend, DeviceOwnerAuthResponse,
};
use block2::RcBlock;
use objc2::runtime::Bool;
use objc2_foundation::{NSError, NSString};
use objc2_local_authentication::{LAContext, LAPolicy};
use std::pin::Pin;
use std::sync::{Arc, RwLock, mpsc};
use std::task::{Context, Poll, Waker};
use std::thread;

#[must_use]
pub const fn response_from(reply: Option<bool>) -> DeviceOwnerAuthResponse {
    match reply {
        None => DeviceOwnerAuthResponse::Unavailable,
        Some(true) => DeviceOwnerAuthResponse::Authenticated,
        Some(false) => DeviceOwnerAuthResponse::Denied,
    }
}

fn evaluate_device_owner(reason: &str) -> DeviceOwnerAuthResponse {
    if reason.trim().is_empty() {
        return DeviceOwnerAuthResponse::Unavailable;
    }
    let policy = LAPolicy::DeviceOwnerAuthentication;
    #[expect(
        unsafe_code,
        reason = "O-07: LAContext 생성은 objc2 바인딩에서 unsafe로 표시되지만 인자가 없다"
    )]
    let context = unsafe { LAContext::new() };
    #[expect(
        unsafe_code,
        reason = "O-07: canEvaluatePolicy:error:는 살아 있는 LAContext와 상수 정책만 받는다"
    )]
    let available = unsafe { context.canEvaluatePolicy_error(policy) }.is_ok();
    if !available {
        return DeviceOwnerAuthResponse::Unavailable;
    }
    let (sender, receiver) = mpsc::sync_channel::<bool>(1);
    let reply = RcBlock::new(move |success: Bool, _error: *mut NSError| {
        let _delivered: Result<(), mpsc::TrySendError<bool>> = sender.try_send(success.as_bool());
    });
    let message = NSString::from_str(reason);
    #[expect(
        unsafe_code,
        reason = "O-07: 응답 블록은 Send인 채널 송신자만 잡고, 응답이 올 때까지 context를 붙잡는다"
    )]
    let () = unsafe { context.evaluatePolicy_localizedReason_reply(policy, &message, &reply) };
    response_from(receiver.recv().ok())
}

type SharedWaker = Arc<RwLock<Option<Waker>>>;

#[derive(Debug)]
struct PendingEvaluation {
    receiver: Option<mpsc::Receiver<DeviceOwnerAuthResponse>>,
    waker: SharedWaker,
}

impl Future for PendingEvaluation {
    type Output = DeviceOwnerAuthResponse;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Some(receiver) = &self.receiver else {
            return Poll::Ready(DeviceOwnerAuthResponse::Unavailable);
        };
        match self.waker.write() {
            Ok(mut slot) => *slot = Some(cx.waker().clone()),
            Err(_poisoned) => return Poll::Ready(DeviceOwnerAuthResponse::Unavailable),
        }
        match receiver.try_recv() {
            Ok(response) => Poll::Ready(response),
            Err(mpsc::TryRecvError::Empty) => Poll::Pending,
            Err(mpsc::TryRecvError::Disconnected) => {
                Poll::Ready(DeviceOwnerAuthResponse::Unavailable)
            }
        }
    }
}

fn evaluate_off_thread(reason: String) -> PendingEvaluation {
    let (sender, receiver) = mpsc::sync_channel(1);
    let waker: SharedWaker = Arc::new(RwLock::new(None));
    let worker_waker = Arc::clone(&waker);
    let spawned = thread::Builder::new()
        .name(String::from("device-owner-auth"))
        .spawn(move || {
            let response = evaluate_device_owner(&reason);
            let _delivered: Result<(), mpsc::TrySendError<DeviceOwnerAuthResponse>> =
                sender.try_send(response);
            if let Ok(slot) = worker_waker.read()
                && let Some(waiting) = slot.as_ref()
            {
                waiting.wake_by_ref();
            }
        });
    PendingEvaluation {
        receiver: spawned.ok().map(|_worker| receiver),
        waker,
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct LocalDeviceOwnerAuth;

impl DeviceOwnerAuthBackend for LocalDeviceOwnerAuth {
    fn evaluate(
        &self,
        reason: &AuthenticationReason,
    ) -> impl Future<Output = DeviceOwnerAuthResponse> + Send {
        evaluate_off_thread(String::from(reason.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::{evaluate_device_owner, evaluate_off_thread, response_from};
    use crate::order_presence::DeviceOwnerAuthResponse;

    #[test]
    fn only_a_successful_reply_authenticates() {
        for (reply, expected) in [
            (Some(true), DeviceOwnerAuthResponse::Authenticated),
            (Some(false), DeviceOwnerAuthResponse::Denied),
            (None, DeviceOwnerAuthResponse::Unavailable),
        ] {
            assert_eq!(response_from(reply), expected, "{reply:?}");
        }
    }

    #[test]
    fn a_blank_reason_never_reaches_the_system_prompt() {
        assert_eq!(
            evaluate_device_owner("   "),
            DeviceOwnerAuthResponse::Unavailable
        );
    }

    #[test]
    fn the_off_thread_evaluation_answers_through_its_future() {
        let pending = evaluate_off_thread(String::from(" "));
        let response = block_on(pending);
        assert_eq!(response, DeviceOwnerAuthResponse::Unavailable);
    }

    fn block_on<F: Future>(unpinned: F) -> F::Output {
        let mut future = std::pin::pin!(unpinned);
        let (sender, woken) = std::sync::mpsc::channel();
        let waker = std::task::Waker::from(std::sync::Arc::new(WakeSignal(sender)));
        let mut context = std::task::Context::from_waker(&waker);
        loop {
            if let std::task::Poll::Ready(output) = future.as_mut().poll(&mut context) {
                return output;
            }
            woken.recv().expect("깨움 신호");
        }
    }

    struct WakeSignal(std::sync::mpsc::Sender<()>);

    impl std::task::Wake for WakeSignal {
        fn wake(self: std::sync::Arc<Self>) {
            self.0.send(()).expect("깨움 전달");
        }
    }
}
