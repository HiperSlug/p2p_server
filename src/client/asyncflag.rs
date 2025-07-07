use std::sync::Arc;

use tokio::sync::RwLock;

use crate::client::ThreadSafe;

pub struct AsyncFlag<T> 
{
	last_poll: T,
	thread_safe: ThreadSafe<T>,
	signal: String,
}

impl<T> AsyncFlag<Option<T>> 
where 
	T: PartialEq + Clone
{
	pub fn new_opt(signal: &str) -> Self {
		let signal = signal.to_string();
		
		Self {
			last_poll: None,
			thread_safe: Arc::new(RwLock::new(None)),
			signal,
		}
	}

	pub fn poll(&mut self) -> Option<(&String, Option<T>)> {
		let Ok(val) = self.thread_safe.try_read() else { return None };
		
		if self.last_poll.as_ref() != (*val).as_ref() {
			self.last_poll = val.clone();
			Some((&self.signal, val.clone()))
		} else {
			None
		}
	}
}

impl AsyncFlag<bool> {
	pub fn new_bool(signal: &str, inital: bool) -> Self {
		let signal = signal.to_string();
		
		Self {
			last_poll: inital,
			thread_safe: Arc::new(RwLock::new(inital)),
			signal,
		}
	}
	
	pub fn poll(&mut self) -> Option<(&String, bool)> {
		let Ok(val) = self.thread_safe.try_read() else { return None };
		
		if self.last_poll != *val {
			self.last_poll = *val;
			Some((&self.signal, *val))
		} else {
			None
		}
	}
}

impl<T> AsyncFlag<T> {
	pub fn inner(&self) -> &ThreadSafe<T> { &self.thread_safe }
}