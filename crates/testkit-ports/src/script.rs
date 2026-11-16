use std::{
    collections::VecDeque,
    future::poll_fn,
    sync::{RwLock, RwLockWriteGuard},
    task::Poll,
};
#[derive(Debug)]
pub(crate) struct State<T>(RwLock<T>);
impl<T> State<T> {
    pub(crate) const fn new(value: T) -> Self {
        Self(RwLock::new(value))
    }
    pub(crate) fn write(&self) -> RwLockWriteGuard<'_, T> {
        match self.0.write() {
            Ok(value) => value,
            Err(poison) => poison.into_inner(),
        }
    }
}
impl<T: Default> Default for State<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}
#[derive(Debug)]
struct ScriptState<I, O> {
    remaining: VecDeque<O>,
    calls: Vec<I>,
}
#[derive(Debug)]
pub struct Script<I, O>(State<ScriptState<I, O>>);
impl<I, O> Default for Script<I, O> {
    fn default() -> Self {
        Self::new([])
    }
}
impl<I, O> Script<I, O> {
    pub fn new<Replies: IntoIterator<Item = O>>(replies: Replies) -> Self {
        Self(State::new(ScriptState {
            remaining: replies.into_iter().collect(),
            calls: Vec::new(),
        }))
    }
    pub fn push(&self, reply: O) {
        self.0.write().remaining.push_back(reply);
    }
    pub fn calls(&self) -> Vec<I>
    where
        I: Clone,
    {
        self.0.write().calls.clone()
    }
    pub async fn run(&self, input: I) -> O
    where
        I: Send + Sync,
        O: Send + Sync,
    {
        let mut reply = {
            let mut state = self.0.write();
            state.calls.push(input);
            state.remaining.pop_front()
        };
        poll_fn(move |_context| {
            assert!(
                reply.is_some(),
                "대본 없음: 호출에 대응하는 결과를 먼저 등록해야 합니다"
            );
            match reply.take() {
                Some(value) => Poll::Ready(value),
                None => Poll::Pending,
            }
        })
        .await
    }
}
