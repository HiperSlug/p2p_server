use std::sync::Arc;
use tokio::sync::RwLock;
use super::ThreadSafe;

pub struct AsyncValue<T> 
{
	last_poll: T,
	thread_safe: ThreadSafe<T>,
	signal: String,
}

impl<T> AsyncValue<T>
where
	T: PartialEq + Clone + Default
{
	pub fn from_default(signal: &str) -> Self {
		Self {
			last_poll: T::default(),
			thread_safe: Arc::new(RwLock::new(T::default())),
			signal: signal.to_string(),
		}
	}

	pub fn poll(&mut self) -> Option<(&String, T)> {
		let Ok(val) = self.thread_safe.try_read() else { return None };
		
		if self.last_poll != (*val) {
			self.last_poll = val.clone();
			Some((&self.signal, val.clone()))
		} else {
			None
		}
	}

	pub fn inner(&self) -> &ThreadSafe<T> { &self.thread_safe }
}