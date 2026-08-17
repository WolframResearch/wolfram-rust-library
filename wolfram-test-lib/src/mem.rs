use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicI64, Ordering};
use wolfram_export::export;

/// Wraps the system allocator, tallying every byte requested via `alloc`
/// (regardless of whether it's later freed) so `mem_allocated` can report
/// total allocation churn for a batch of calls — the metric that separates
/// zero-copy MArgument access (native/margs) from marshaling paths (wstp/wxf)
/// that must build owned Rust values from the wire.
struct TrackingAllocator;

static ALLOCATED: AtomicI64 = AtomicI64::new(0);

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size() as i64, Ordering::Relaxed);
        System.alloc(layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size() as i64, Ordering::Relaxed);
        System.alloc_zeroed(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if new_size > layout.size() {
            ALLOCATED.fetch_add((new_size - layout.size()) as i64, Ordering::Relaxed);
        }
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

/// Zero the running total and return whatever it held before the reset.
#[export]
fn mem_reset() -> i64 {
    ALLOCATED.swap(0, Ordering::Relaxed)
}

/// Bytes allocated since the last `mem_reset` call.
#[export]
fn mem_allocated() -> i64 {
    ALLOCATED.load(Ordering::Relaxed)
}
