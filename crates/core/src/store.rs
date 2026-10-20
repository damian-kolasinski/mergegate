//! What the store ports have in common. Each slice defines its own store
//! trait next to its domain types, e.g. [`crate::project::ProjectStore`].

/// A store adapter's failure. The domain can't recover from one, only report
/// it.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct StoreError(Box<dyn std::error::Error + Send + Sync>);

impl StoreError {
    pub fn new(source: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self(source.into())
    }
}

/// A store other work goes on in while a task works through it: the task
/// takes it for one step at a time, so what it does between steps, as a
/// merge's calls to GitHub, holds up no one.
pub trait Shared<S> {
    fn with<T>(&self, step: impl FnOnce(&mut S) -> T) -> T;
}

/// A store nothing else uses, as in tests: every step has it to itself.
impl<S> Shared<S> for std::cell::RefCell<&mut S> {
    fn with<T>(&self, step: impl FnOnce(&mut S) -> T) -> T {
        step(&mut self.borrow_mut())
    }
}
