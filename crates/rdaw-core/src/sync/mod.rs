pub mod ipc_ring;
pub mod ring;
pub mod shared_mem;
pub mod spsc;

pub unsafe trait IpcSafe {}

unsafe impl IpcSafe for u8 {}
