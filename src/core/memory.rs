use std::alloc::{GlobalAlloc, System, Layout};
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct TrackingAllocator {
    allocator: System,
    allocated_bytes: AtomicUsize,
}

#[global_allocator]
pub static ALLOCATOR: TrackingAllocator = TrackingAllocator::new();

impl TrackingAllocator {
    pub const fn new() -> Self {
        Self {
            allocator: System,
            allocated_bytes: AtomicUsize::new(0),
        }
    }

    pub fn get_current_memory_usage(&self) -> usize {
        self.allocated_bytes.load(Ordering::Relaxed)
    }
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = self.allocator.alloc(layout);
        if !ptr.is_null() {
            self.allocated_bytes.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.allocator.dealloc(ptr, layout);
        self.allocated_bytes.fetch_sub(layout.size(), Ordering::Relaxed);
    }
    
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = self.allocator.alloc_zeroed(layout);
        if !ptr.is_null() {
            self.allocated_bytes.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = self.allocator.realloc(ptr, layout, new_size);
        if !new_ptr.is_null() {
            if new_size > layout.size() {
                self.allocated_bytes.fetch_add(new_size - layout.size(), Ordering::Relaxed);
            } else {
                self.allocated_bytes.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        new_ptr
    }
}
