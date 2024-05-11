mod named_event;
pub mod ring;
mod shared_mem;
pub mod spsc;

pub use self::named_event::NamedEvent;
pub use self::shared_mem::SharedMemory;

/// Marker trait for types which can be safely shared between processes
///
/// # Safety
///
/// By implementing this trait you guarantee that the type doesn't contain any pointers to possibly
/// non shared memory.
pub unsafe trait IpcSafe {}

unsafe impl IpcSafe for u8 {}

unsafe impl IpcSafe for f32 {}
