#![no_std]

use allocator::{AllocError, AllocResult, BaseAllocator, ByteAllocator, PageAllocator};
use core::alloc::Layout;
use core::ptr::NonNull;

/// Early memory allocator
/// Use it before formal bytes-allocator and pages-allocator can work!
/// This is a double-end memory range:
/// - Alloc bytes forward
/// - Alloc pages backward
///
/// [ bytes-used | avail-area | pages-used ]
/// |            | -->    <-- |            |
/// start       b_pos        p_pos       end
///
/// For bytes area, 'count' records number of allocations.
/// When it goes down to ZERO, free bytes-used area.
/// For pages area, it will never be freed!
///

/// 一般将0x80000000 - 0x80200000这段内存进行分配。
/// 在最开始初始化的那段时间，需要使用动态内存分配器。等过完那段时间，这段内存会释放掉，开启新的内存分配器。
pub struct EarlyAllocator<const PAGE_SIZE: usize> {
    base: usize,
    total_size: usize,
    b_pos: usize,
    p_pos: usize,
    used_bytes: usize,
    used_pages: usize,
}

impl<const PAGE_SIZE: usize> EarlyAllocator<PAGE_SIZE> {
    pub const fn new() -> Self {
        Self {
            base: 0,
            total_size: 0,
            b_pos: 0,
            p_pos: 0,
            used_bytes: 0,
            used_pages: 0,
        }
    }
}

/// 一般最大空间为2MB
impl<const PAGE_SIZE: usize> BaseAllocator for EarlyAllocator<PAGE_SIZE> {
    fn init(&mut self, start: usize, size: usize) {
        assert!(PAGE_SIZE.is_power_of_two());

        let start = (start.wrapping_add(PAGE_SIZE - 1)) & !(PAGE_SIZE - 1);
        let end = (start + size) & !(PAGE_SIZE - 1);

        self.base = start;
        self.total_size = end - start;
        self.b_pos = start;
        self.p_pos = end;
    }

    fn add_memory(&mut self, _start: usize, _size: usize) -> AllocResult {
        Err(AllocError::NoMemory)
    }
}

impl<const PAGE_SIZE: usize> ByteAllocator for EarlyAllocator<PAGE_SIZE> {
    fn alloc(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        if layout.size() > (self.p_pos - self.b_pos) {
            return Err(AllocError::NoMemory);
        }

        let data = NonNull::new(self.b_pos as *mut u8).unwrap();
        self.b_pos = self.b_pos.saturating_add(layout.size());

        self.used_bytes += layout.size();
        Ok(data)
    }

    fn dealloc(&mut self, pos: NonNull<u8>, layout: Layout) {
        let pos = pos.as_ptr() as usize;
        if pos < self.base || pos >= self.p_pos {
            return;
        }

        let size = layout.size();
        if size > (self.p_pos - self.b_pos) {
            return;
        }

        self.used_bytes = self.used_bytes.saturating_sub(size);        

        if self.used_bytes == 0 {
            self.b_pos = self.base;
        }
    }

    fn total_bytes(&self) -> usize {
        self.total_size
    }

    fn used_bytes(&self) -> usize {
        self.used_bytes
    }

    fn available_bytes(&self) -> usize {
        self.p_pos.saturating_sub(self.b_pos)
    }
}

impl<const PAGE_SIZE: usize> PageAllocator for EarlyAllocator<PAGE_SIZE> {
    const PAGE_SIZE: usize = PAGE_SIZE;

    fn alloc_pages(&mut self, num_pages: usize, align_pow2: usize) -> AllocResult<usize> {
        if align_pow2 % PAGE_SIZE != 0 {
            return Err(AllocError::InvalidParam);
        }
        let align_pow2 = align_pow2 / PAGE_SIZE;
        if !align_pow2.is_power_of_two() {
            return Err(AllocError::InvalidParam);
        }

        let need_bytes = num_pages * PAGE_SIZE;
        if need_bytes > self.available_pages() * PAGE_SIZE {
            return Err(AllocError::NoMemory);
        }

        self.p_pos = self.p_pos.saturating_sub(need_bytes);
        self.used_pages += num_pages;

        Ok(self.p_pos)
    }

    fn dealloc_pages(&mut self, _pos: usize, _num_pages: usize) {
        unreachable!("EarlyAllocator does not support deallocation");
    }

    /// Returns the total number of memory pages.
    fn total_pages(&self) -> usize {
        self.total_size / PAGE_SIZE
    }

    /// Returns the number of allocated memory pages.
    fn used_pages(&self) -> usize {
        self.used_pages
    }

    /// Returns the number of available memory pages.
    fn available_pages(&self) -> usize {
        self.p_pos.saturating_sub(self.b_pos) / PAGE_SIZE
    }
}
