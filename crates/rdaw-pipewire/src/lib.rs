mod error;
mod internal;

use rdaw_core::driver::{self, OutStreamDesc};

pub use crate::error::{Error, Result};
use crate::internal::{Handle, OutStreamId, PwThread};

pub struct Driver {
    handle: Handle,
}

impl Driver {
    pub fn new() -> Result<Driver> {
        let (handle, receiver) = Handle::new();

        let (err_sender, err_receiver) = oneshot::channel();

        std::thread::Builder::new()
            .name("pipewire-driver".into())
            .spawn(move || match PwThread::new() {
                Ok(thread) => {
                    let _ = err_sender.send(None);
                    thread.run(receiver);
                }
                Err(e) => {
                    let _ = err_sender.send(Some(e));
                }
            })
            .map_err(Error::ThreadSpawn)?;

        if let Ok(Some(err)) = err_receiver.recv() {
            return Err(err);
        }

        Ok(Driver { handle })
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        let _ = self.handle.terminate();
    }
}

impl driver::Driver for Driver {
    type Error = Error;
    type OutStream = OutStream;

    fn create_out_stream(&self, desc: OutStreamDesc) -> Result<OutStream> {
        let id = self.handle.create_out_stream(desc)?;
        Ok(OutStream {
            id,
            handle: self.handle.clone(),
        })
    }
}

pub struct OutStream {
    id: OutStreamId,
    handle: Handle,
}

impl driver::OutStream for OutStream {
    type Error = Error;

    fn is_active(&self) -> Result<bool> {
        self.handle.is_out_stream_active(self.id)
    }

    fn set_active(&self, active: bool) -> Result<()> {
        self.handle.set_out_stream_active(self.id, active)
    }
}

impl Drop for OutStream {
    fn drop(&mut self) {
        let _ = self.handle.destroy_out_stream(self.id);
    }
}
