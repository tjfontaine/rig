use bytes::Bytes;
use std::pin::Pin;

use futures::Stream;

// WasmCompatSend - does not require Send in WASM environments
#[cfg(not(any(
    all(feature = "wasm", target_arch = "wasm32"),
    all(feature = "wasip2", target_arch = "wasm32")
)))]
pub trait WasmCompatSend: Send {}

#[cfg(any(
    all(feature = "wasm", target_arch = "wasm32"),
    all(feature = "wasip2", target_arch = "wasm32")
))]
pub trait WasmCompatSend {}

#[cfg(not(any(
    all(feature = "wasm", target_arch = "wasm32"),
    all(feature = "wasip2", target_arch = "wasm32")
)))]
impl<T> WasmCompatSend for T where T: Send {}

#[cfg(any(
    all(feature = "wasm", target_arch = "wasm32"),
    all(feature = "wasip2", target_arch = "wasm32")
))]
impl<T> WasmCompatSend for T {}

// WasmCompatSendStream - stream variant
#[cfg(not(any(
    all(feature = "wasm", target_arch = "wasm32"),
    all(feature = "wasip2", target_arch = "wasm32")
)))]
pub trait WasmCompatSendStream:
    Stream<Item = Result<Bytes, crate::http_client::Error>> + Send
{
    type InnerItem: Send;
}

#[cfg(any(
    all(feature = "wasm", target_arch = "wasm32"),
    all(feature = "wasip2", target_arch = "wasm32")
))]
pub trait WasmCompatSendStream: Stream<Item = Result<Bytes, crate::http_client::Error>> {
    type InnerItem;
}

#[cfg(not(any(
    all(feature = "wasm", target_arch = "wasm32"),
    all(feature = "wasip2", target_arch = "wasm32")
)))]
impl<T> WasmCompatSendStream for T
where
    T: Stream<Item = Result<Bytes, crate::http_client::Error>> + Send,
{
    type InnerItem = Result<Bytes, crate::http_client::Error>;
}

#[cfg(any(
    all(feature = "wasm", target_arch = "wasm32"),
    all(feature = "wasip2", target_arch = "wasm32")
))]
impl<T> WasmCompatSendStream for T
where
    T: Stream<Item = Result<Bytes, crate::http_client::Error>>,
{
    type InnerItem = Result<Bytes, crate::http_client::Error>;
}

// WasmCompatSync - does not require Sync in WASM environments
#[cfg(not(any(
    all(feature = "wasm", target_arch = "wasm32"),
    all(feature = "wasip2", target_arch = "wasm32")
)))]
pub trait WasmCompatSync: Sync {}

#[cfg(any(
    all(feature = "wasm", target_arch = "wasm32"),
    all(feature = "wasip2", target_arch = "wasm32")
))]
pub trait WasmCompatSync {}

#[cfg(not(any(
    all(feature = "wasm", target_arch = "wasm32"),
    all(feature = "wasip2", target_arch = "wasm32")
)))]
impl<T> WasmCompatSync for T where T: Sync {}

#[cfg(any(
    all(feature = "wasm", target_arch = "wasm32"),
    all(feature = "wasip2", target_arch = "wasm32")
))]
impl<T> WasmCompatSync for T {}

// WasmBoxedFuture - boxed future type alias
#[cfg(not(target_family = "wasm"))]
pub type WasmBoxedFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[cfg(target_family = "wasm")]
pub type WasmBoxedFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

// Helper macros for conditional compilation
#[macro_export]
macro_rules! if_wasm {
    ($($tokens:tt)*) => {
        #[cfg(any(
            all(feature = "wasm", target_arch = "wasm32"),
            all(feature = "wasip2", target_arch = "wasm32")
        ))]
        $($tokens)*

    };
}

#[macro_export]
macro_rules! if_not_wasm {
    ($($tokens:tt)*) => {
        #[cfg(not(any(
            all(feature = "wasm", target_arch = "wasm32"),
            all(feature = "wasip2", target_arch = "wasm32")
        )))]
        $($tokens)*

    };
}

// ============================================================================
// WASIP2/WASM-compatible sync primitives
// ============================================================================
//
// MOTIVATION: WASIP2 + JSPI (JavaScript Promise Integration)
//
// In WASIP2 environments running in browsers with JSPI, the WASM module can
// suspend its stack when calling blocking WASI imports (like `blocking_read`
// or `poll.block()`). This allows the JavaScript event loop to remain
// responsive while the WASM code "blocks" on I/O.
//
// However, `tokio::sync` primitives (RwLock, watch, etc.) require a tokio
// runtime for their waker/notification mechanisms. When used with
// `futures::executor::block_on` in WASM:
//
// 1. The future returns `Poll::Pending` (e.g., waiting for RwLock)
// 2. `block_on` tries to park the thread waiting for a waker
// 3. Thread parking doesn't work in single-threaded WASM
// 4. DEADLOCK!
//
// SOLUTION: For WASIP2 builds, we use `std::sync` equivalents which are
// synchronous and work correctly in single-threaded WASM. The blocking
// nature is acceptable because:
// - WASM is single-threaded, so no real contention occurs
// - JSPI handles the actual async suspension at I/O boundaries
// - We only need the locking semantics, not async notification

// ============================================================================
// Native: Use tokio::sync::RwLock directly
// ============================================================================

#[cfg(not(any(
    all(feature = "wasm", target_arch = "wasm32"),
    all(feature = "wasip2", target_arch = "wasm32")
)))]
pub use tokio::sync::RwLock as WasmRwLock;

#[cfg(not(any(
    all(feature = "wasm", target_arch = "wasm32"),
    all(feature = "wasip2", target_arch = "wasm32")
)))]
pub type WasmRwLockReadGuard<'a, T> = tokio::sync::RwLockReadGuard<'a, T>;

#[cfg(not(any(
    all(feature = "wasm", target_arch = "wasm32"),
    all(feature = "wasip2", target_arch = "wasm32")
)))]
pub type WasmRwLockWriteGuard<'a, T> = tokio::sync::RwLockWriteGuard<'a, T>;

// ============================================================================
// WASM/WASIP2: Newtype wrapper with async-compatible interface
// ============================================================================
//
// This wrapper around std::sync::RwLock provides .read() and .write() methods
// that return immediately-ready futures. This allows code using ".read().await"
// and ".write().await" to work in WASIP2 without a tokio runtime.
//
// The futures are trivial - they just block on acquisition and return Ready.
// This is fine in WASM because:
// 1. WASM is single-threaded, so no actual contention
// 2. Blocking is effectively instant
// 3. We just need the API compatibility

#[cfg(any(
    all(feature = "wasm", target_arch = "wasm32"),
    all(feature = "wasip2", target_arch = "wasm32")
))]
pub type WasmRwLockReadGuard<'a, T> = std::sync::RwLockReadGuard<'a, T>;

#[cfg(any(
    all(feature = "wasm", target_arch = "wasm32"),
    all(feature = "wasip2", target_arch = "wasm32")
))]
pub type WasmRwLockWriteGuard<'a, T> = std::sync::RwLockWriteGuard<'a, T>;

/// RwLock wrapper for WASM that provides async-compatible interface.
///
/// This wraps std::sync::RwLock but provides .read() and .write() methods
/// that return futures (immediately ready) for compatibility with async code.
#[cfg(any(
    all(feature = "wasm", target_arch = "wasm32"),
    all(feature = "wasip2", target_arch = "wasm32")
))]
pub struct WasmRwLock<T> {
    inner: std::sync::RwLock<T>,
}

#[cfg(any(
    all(feature = "wasm", target_arch = "wasm32"),
    all(feature = "wasip2", target_arch = "wasm32")
))]
impl<T> WasmRwLock<T> {
    /// Create a new WasmRwLock
    pub fn new(value: T) -> Self {
        Self {
            inner: std::sync::RwLock::new(value),
        }
    }

    /// Async-compatible read lock acquisition.
    /// Returns an immediately-ready future for API compatibility with tokio.
    pub async fn read(&self) -> WasmRwLockReadGuard<'_, T> {
        // In WASM, this is effectively synchronous.
        // The "async" here is purely for API compatibility.
        self.inner.read().expect("RwLock poisoned")
    }

    /// Async-compatible write lock acquisition.
    /// Returns an immediately-ready future for API compatibility with tokio.
    pub async fn write(&self) -> WasmRwLockWriteGuard<'_, T> {
        // In WASM, this is effectively synchronous.
        // The "async" here is purely for API compatibility.
        self.inner.write().expect("RwLock poisoned")
    }
}
