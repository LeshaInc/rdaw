use std::io;
use std::time::Duration;

use self::unix::OsEvent;

#[cfg(unix)]
mod unix;

/// Event object for notifying other processes.
pub struct NamedEvent(OsEvent);

impl NamedEvent {
    /// Creates an event object with a specified ID prefix.
    ///
    /// The rest of the ID will be randomly generated.
    pub fn create(prefix: &str) -> io::Result<NamedEvent> {
        OsEvent::create(prefix).map(NamedEvent)
    }

    /// Opens an event object by ID.
    ///
    /// # Safety
    ///
    /// ID must be obtained by [`NamedEvent::id`]
    pub unsafe fn open(id: &str) -> io::Result<NamedEvent> {
        OsEvent::open(id).map(NamedEvent)
    }

    pub fn id(&self) -> &str {
        self.0.id()
    }

    pub fn prefix(&self) -> &str {
        self.0.prefix()
    }

    pub fn wait(&self) {
        self.0.wait()
    }

    pub fn wait_timeout(&self, timeout: Duration) {
        self.0.wait_timeout(timeout)
    }

    pub fn signal(&self) {
        self.0.signal()
    }
}
