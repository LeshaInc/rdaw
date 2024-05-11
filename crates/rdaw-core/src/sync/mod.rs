pub mod ipc_event;
pub mod ipc_ring;
pub mod ipc_spsc;
pub mod ring;
pub mod shared_mem;
pub mod spsc;

/// Marker trait for types which can be safely shared between processes
///
/// # Safety
///
/// By implementing this trait you guarantee that the type doesn't contain any pointers to possibly
/// non shared memory.
pub unsafe trait IpcSafe {}

unsafe impl IpcSafe for u8 {}

unsafe impl IpcSafe for f32 {}
