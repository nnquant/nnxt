//! 共享内存 SPMC 环形缓冲区实现。

use std::ffi::CString;
use std::marker::PhantomData;
use std::mem::{align_of, size_of};
use std::os::fd::RawFd;
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::address::Address;
use crate::error::Error;
use crate::DEFAULT_MAX_READERS;

const MAGIC: u64 = 0x5250_444D_5155_4555; // "RPDMQUEU"
const VERSION: u32 = 1;
const FREE_READER: u64 = u64::MAX;

#[repr(C, align(64))]
struct Header {
    magic: u64,
    version: u32,
    _reserved: u32,
    capacity: u64,
    element_size: u64,
    max_readers: u64,
    write_seq: AtomicU64,
}

impl Header {
    fn new(capacity: usize, element_size: usize, max_readers: usize) -> Self {
        Self {
            magic: MAGIC,
            version: VERSION,
            _reserved: 0,
            capacity: capacity as u64,
            element_size: element_size as u64,
            max_readers: max_readers as u64,
            write_seq: AtomicU64::new(0),
        }
    }
}

#[derive(Clone, Copy)]
struct Layout {
    readers_offset: usize,
    data_offset: usize,
    total_size: usize,
}

fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

fn layout_for<T>(capacity: usize, max_readers: usize) -> Result<Layout, Error> {
    if capacity == 0 {
        return Err(Error::CapacityTooSmall);
    }
    let header_size = size_of::<Header>();
    let readers_offset = align_up(header_size, align_of::<AtomicU64>());
    let readers_size = size_of::<AtomicU64>()
        .checked_mul(max_readers)
        .ok_or(Error::CapacityTooSmall)?;
    let data_offset = align_up(readers_offset + readers_size, align_of::<T>());
    let data_size = size_of::<T>()
        .checked_mul(capacity)
        .ok_or(Error::CapacityTooSmall)?;
    let total_size = data_offset
        .checked_add(data_size)
        .ok_or(Error::CapacityTooSmall)?;
    Ok(Layout {
        readers_offset,
        data_offset,
        total_size,
    })
}

struct OpenedShm {
    fd: RawFd,
    used_shm_open: bool,
}

fn errno() -> i32 {
    std::io::Error::last_os_error()
        .raw_os_error()
        .unwrap_or(0)
}

fn open_create_shm(addr: &Address) -> Result<OpenedShm, Error> {
    let name = CString::new(addr.shm_name()).map_err(|_| Error::ShmOpenFailed { errno: 0 })?;
    let fd = unsafe { libc::shm_open(name.as_ptr(), libc::O_CREAT | libc::O_EXCL | libc::O_RDWR, 0o600) };
    if fd >= 0 {
        return Ok(OpenedShm {
            fd,
            used_shm_open: true,
        });
    }
    let err = errno();
    if err != libc::EINVAL {
        return Err(Error::ShmOpenFailed { errno: err });
    }

    std::fs::create_dir_all(addr.file_path().parent().unwrap_or_else(|| addr.file_path()))
        .map_err(|_| Error::FileOpenFailed { errno: errno() })?;
    let path = CString::new(addr.file_path().to_string_lossy().as_bytes())
        .map_err(|_| Error::FileOpenFailed { errno: 0 })?;
    let fd = unsafe { libc::open(path.as_ptr(), libc::O_CREAT | libc::O_EXCL | libc::O_RDWR, 0o600) };
    if fd < 0 {
        return Err(Error::FileOpenFailed { errno: errno() });
    }
    Ok(OpenedShm {
        fd,
        used_shm_open: false,
    })
}

fn open_existing_shm(addr: &Address) -> Result<OpenedShm, Error> {
    let name = CString::new(addr.shm_name()).map_err(|_| Error::ShmOpenFailed { errno: 0 })?;
    let fd = unsafe { libc::shm_open(name.as_ptr(), libc::O_RDWR, 0o600) };
    if fd >= 0 {
        return Ok(OpenedShm {
            fd,
            used_shm_open: true,
        });
    }
    let err = errno();
    if err != libc::EINVAL && err != libc::ENOENT {
        return Err(Error::ShmOpenFailed { errno: err });
    }
    let path = CString::new(addr.file_path().to_string_lossy().as_bytes())
        .map_err(|_| Error::FileOpenFailed { errno: 0 })?;
    let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDWR, 0o600) };
    if fd < 0 {
        return Err(Error::NotFound);
    }
    Ok(OpenedShm {
        fd,
        used_shm_open: false,
    })
}

fn shm_unlink(addr: &Address, used_shm_open: bool) -> Result<(), Error> {
    if used_shm_open {
        let name = CString::new(addr.shm_name()).map_err(|_| Error::ShmUnlinkFailed { errno: 0 })?;
        let rc = unsafe { libc::shm_unlink(name.as_ptr()) };
        if rc != 0 {
            return Err(Error::ShmUnlinkFailed { errno: errno() });
        }
        return Ok(());
    }
    let path = CString::new(addr.file_path().to_string_lossy().as_bytes())
        .map_err(|_| Error::FileUnlinkFailed { errno: 0 })?;
    let rc = unsafe { libc::unlink(path.as_ptr()) };
    if rc != 0 {
        return Err(Error::FileUnlinkFailed { errno: errno() });
    }
    Ok(())
}

fn map_shared(fd: RawFd, size: usize) -> Result<*mut u8, Error> {
    let ptr = unsafe {
        libc::mmap(
            ptr::null_mut(),
            size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            fd,
            0,
        )
    };
    if ptr == libc::MAP_FAILED {
        return Err(Error::MmapFailed { errno: errno() });
    }
    Ok(ptr as *mut u8)
}

fn unmap_shared(ptr: *mut u8, size: usize) -> Result<(), Error> {
    let rc = unsafe { libc::munmap(ptr as *mut libc::c_void, size) };
    if rc != 0 {
        return Err(Error::MunmapFailed { errno: errno() });
    }
    Ok(())
}

fn close_fd(fd: RawFd) {
    unsafe {
        libc::close(fd);
    }
}

pub struct Writer<T> {
    addr: Address,
    fd: RawFd,
    map: *mut u8,
    map_len: usize,
    header: *mut Header,
    data: *mut T,
    capacity: usize,
    used_shm_open: bool,
    pending_seq: u64,
    has_pending: bool,
    _marker: PhantomData<T>,
}

impl<T: Copy> Writer<T> {
    /// 创建共享内存队列，单生产者写入端。
    ///
    /// :param addr: 队列地址
    /// :type addr: &Address
    /// :param capacity: 环形缓冲区容量
    /// :type capacity: usize
    /// :returns: Writer 实例
    /// :rtype: Writer<T>
    /// :raises Error: 创建或映射共享内存失败
    pub fn create(addr: &Address, capacity: usize) -> Result<Self, Error> {
        Self::create_with_readers(addr, capacity, DEFAULT_MAX_READERS)
    }

    pub fn create_with_readers(
        addr: &Address,
        capacity: usize,
        max_readers: usize,
    ) -> Result<Self, Error> {
        let layout = layout_for::<T>(capacity, max_readers)?;
        let opened = open_create_shm(addr).map_err(|err| match err {
            Error::ShmOpenFailed { .. } | Error::FileOpenFailed { .. } => Error::AddressInUse,
            other => other,
        })?;
        let fd = opened.fd;
        // SAFETY: fd 是有效的文件描述符，layout.total_size 是正确计算的大小
        let rc = unsafe { libc::ftruncate(fd, layout.total_size as libc::off_t) };
        if rc != 0 {
            close_fd(fd);
            return Err(Error::TruncateFailed { errno: errno() });
        }
        let map = match map_shared(fd, layout.total_size) {
            Ok(ptr) => ptr,
            Err(err) => {
                let _ = shm_unlink(addr, opened.used_shm_open);
                close_fd(fd);
                return Err(err);
            }
        };
        let header = map as *mut Header;
        // SAFETY: map 指向足够大小的内存区域，Header 是 repr(C) 且对齐到 64 字节
        unsafe {
            ptr::write(header, Header::new(capacity, size_of::<T>(), max_readers));
        }
        // SAFETY: readers_offset 经过对齐计算，指向 Header 之后的有效内存区域
        let readers_ptr = unsafe { map.add(layout.readers_offset) as *mut AtomicU64 };
        for idx in 0..max_readers {
            // SAFETY: idx < max_readers，readers_ptr + idx 在分配的内存范围内
            unsafe {
                ptr::write(readers_ptr.add(idx), AtomicU64::new(FREE_READER));
            }
        }
        // SAFETY: data_offset 经过对齐计算，指向 readers 区域之后的有效内存
        let data = unsafe { map.add(layout.data_offset) as *mut T };
        // SAFETY: 初始化数据区域为零，大小经过 checked_mul 验证
        unsafe {
            ptr::write_bytes(data as *mut u8, 0, size_of::<T>() * capacity);
        }
        Ok(Self {
            addr: addr.clone(),
            fd,
            map,
            map_len: layout.total_size,
            header,
            data,
            capacity,
            used_shm_open: opened.used_shm_open,
            pending_seq: 0,
            has_pending: false,
            _marker: PhantomData,
        })
    }

    /// 获取下一个槽位可写引用（零拷贝）。
    pub fn prepare(&mut self) -> &mut T {
        // SAFETY: self.header 在 create 时初始化，指向有效的 Header
        let seq = unsafe { (*self.header).write_seq.load(Ordering::Relaxed) };
        let slot = (seq % self.capacity as u64) as usize;
        self.pending_seq = seq;
        self.has_pending = true;
        // SAFETY: slot < capacity，self.data + slot 在数据区域范围内
        unsafe { &mut *self.data.add(slot) }
    }

    /// 提交写入，使数据对 Reader 可见。
    pub fn commit(&mut self) {
        if !self.has_pending {
            return;
        }
        let next = self.pending_seq + 1;
        // SAFETY: self.header 在 create 时初始化，指向有效的 Header
        unsafe {
            (*self.header).write_seq.store(next, Ordering::Release);
        }
        self.has_pending = false;
    }

    /// 直接写入一个值（非零拷贝便利接口）。
    pub fn write(&mut self, value: T) {
        let slot = self.prepare();
        *slot = value;
        self.commit();
    }
}

impl<T> Drop for Writer<T> {
    fn drop(&mut self) {
        let _ = unmap_shared(self.map, self.map_len);
        let _ = shm_unlink(&self.addr, self.used_shm_open);
        close_fd(self.fd);
    }
}

// SAFETY: Writer 可以安全地在线程间转移所有权，因为：
// 1. 共享内存映射在所有线程中具有相同的地址
// 2. Writer 是单生产者，同一时间只有一个线程持有
// 3. 内部状态 (pending_seq, has_pending) 不跨线程共享
unsafe impl<T: Send> Send for Writer<T> {}

pub struct Reader<T> {
    #[allow(dead_code)]
    addr: Address,
    fd: RawFd,
    map: *mut u8,
    map_len: usize,
    header: *mut Header,
    data: *mut T,
    read_slot: *mut AtomicU64,
    #[allow(dead_code)]
    used_shm_open: bool,
    _marker: PhantomData<T>,
}

impl<T: Copy> Reader<T> {
    /// 连接到共享内存队列（非阻塞）。
    ///
    /// :param addr: 队列地址
    /// :type addr: &Address
    /// :returns: Reader 实例
    /// :rtype: Reader<T>
    /// :raises Error: 队列不存在或初始化失败
    pub fn connect(addr: &Address) -> Result<Self, Error> {
        let opened = open_existing_shm(addr)?;
        let fd = opened.fd;
        // SAFETY: stat 被零初始化，fstat 会填充有效数据
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        // SAFETY: fd 是有效的文件描述符，stat 是有效的可写指针
        let rc = unsafe { libc::fstat(fd, &mut stat) };
        if rc != 0 {
            close_fd(fd);
            return Err(Error::FileStatFailed { errno: errno() });
        }
        let map_len = stat.st_size as usize;
        let map = match map_shared(fd, map_len) {
            Ok(ptr) => ptr,
            Err(err) => {
                close_fd(fd);
                return Err(err);
            }
        };
        let header = map as *mut Header;
        // SAFETY: map 指向有效的共享内存，Header 由 Writer 正确初始化
        let header_ref = unsafe { &*header };
        if header_ref.magic != MAGIC || header_ref.version != VERSION {
            unmap_shared(map, map_len).ok();
            close_fd(fd);
            return Err(Error::InvalidHeader);
        }
        let element_size = header_ref.element_size as usize;
        if element_size != size_of::<T>() {
            unmap_shared(map, map_len).ok();
            close_fd(fd);
            return Err(Error::SizeMismatch {
                expected: size_of::<T>(),
                actual: element_size,
            });
        }
        let capacity = header_ref.capacity as usize;
        let max_readers = header_ref.max_readers as usize;
        let layout = layout_for::<T>(capacity, max_readers)?;
        if layout.total_size > map_len {
            unmap_shared(map, map_len).ok();
            close_fd(fd);
            return Err(Error::SizeMismatch {
                expected: layout.total_size,
                actual: map_len,
            });
        }
        // SAFETY: readers_offset 由 layout_for 正确计算，在映射范围内
        let readers_ptr = unsafe { map.add(layout.readers_offset) as *mut AtomicU64 };
        let write_seq = header_ref.write_seq.load(Ordering::Acquire);
        let mut slot_ptr: *mut AtomicU64 = ptr::null_mut();
        for idx in 0..max_readers {
            // SAFETY: idx < max_readers，readers_ptr + idx 在分配的内存范围内
            let candidate = unsafe { &*readers_ptr.add(idx) };
            if candidate
                .compare_exchange(FREE_READER, write_seq, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                slot_ptr = readers_ptr.wrapping_add(idx);
                break;
            }
        }
        if slot_ptr.is_null() {
            unmap_shared(map, map_len).ok();
            close_fd(fd);
            return Err(Error::ReaderSlotsFull);
        }
        // SAFETY: data_offset 由 layout_for 正确计算，在映射范围内
        let data = unsafe { map.add(layout.data_offset) as *mut T };
        Ok(Self {
            addr: addr.clone(),
            fd,
            map,
            map_len,
            header,
            data,
            read_slot: slot_ptr,
            used_shm_open: opened.used_shm_open,
            _marker: PhantomData,
        })
    }

    /// 非阻塞读取，若无新数据返回 None。
    pub fn try_read(&mut self) -> Option<&T> {
        // SAFETY: self.header 在 connect 时验证，指向有效的 Header
        let header = unsafe { &*self.header };
        let write_seq = header.write_seq.load(Ordering::Acquire);
        // SAFETY: self.read_slot 在 connect 时分配，指向有效的 AtomicU64
        let mut read_seq = unsafe { &*self.read_slot }.load(Ordering::Relaxed);
        if write_seq == read_seq {
            return None;
        }
        let capacity = header.capacity;
        if write_seq.saturating_sub(read_seq) > capacity {
            read_seq = write_seq.saturating_sub(capacity);
            // SAFETY: self.read_slot 指向有效的 AtomicU64
            unsafe { &*self.read_slot }.store(read_seq, Ordering::Release);
        }
        let slot = (read_seq % capacity) as usize;
        // SAFETY: slot < capacity，self.data + slot 在数据区域范围内
        let value = unsafe { &*self.data.add(slot) };
        let next = read_seq + 1;
        // SAFETY: self.read_slot 指向有效的 AtomicU64
        unsafe { &*self.read_slot }.store(next, Ordering::Release);
        Some(value)
    }

    /// 阻塞读取，直到有新数据可用。
    pub fn read(&mut self) -> &T {
        // SAFETY: self.header 在 connect 时验证，指向有效的 Header
        let header = unsafe { &*self.header };
        let capacity = header.capacity;
        let mut spins = 0u32;

        loop {
            let write_seq = header.write_seq.load(Ordering::Acquire);
            // SAFETY: self.read_slot 在 connect 时分配，指向有效的 AtomicU64
            let read_seq = unsafe { &*self.read_slot }.load(Ordering::Relaxed);

            if write_seq > read_seq {
                let actual_read = if write_seq.saturating_sub(read_seq) > capacity {
                    let new_seq = write_seq.saturating_sub(capacity);
                    // SAFETY: self.read_slot 指向有效的 AtomicU64
                    unsafe { &*self.read_slot }.store(new_seq, Ordering::Release);
                    new_seq
                } else {
                    read_seq
                };

                let slot = (actual_read % capacity) as usize;
                // SAFETY: self.read_slot 指向有效的 AtomicU64
                unsafe { &*self.read_slot }.store(actual_read + 1, Ordering::Release);
                // SAFETY: slot < capacity，self.data + slot 在数据区域范围内
                return unsafe { &*self.data.add(slot) };
            }

            spins = spins.saturating_add(1);
            if spins < 1000 {
                std::hint::spin_loop();
            } else {
                std::thread::yield_now();
            }
        }
    }

    /// 带超时的阻塞读取。
    pub fn read_timeout(&mut self, timeout: Duration) -> Option<&T> {
        // SAFETY: self.header 在 connect 时验证，指向有效的 Header
        let header = unsafe { &*self.header };
        let capacity = header.capacity;
        let start = Instant::now();

        loop {
            let write_seq = header.write_seq.load(Ordering::Acquire);
            // SAFETY: self.read_slot 在 connect 时分配，指向有效的 AtomicU64
            let read_seq = unsafe { &*self.read_slot }.load(Ordering::Relaxed);

            if write_seq > read_seq {
                let actual_read = if write_seq.saturating_sub(read_seq) > capacity {
                    let new_seq = write_seq.saturating_sub(capacity);
                    // SAFETY: self.read_slot 指向有效的 AtomicU64
                    unsafe { &*self.read_slot }.store(new_seq, Ordering::Release);
                    new_seq
                } else {
                    read_seq
                };

                let slot = (actual_read % capacity) as usize;
                // SAFETY: self.read_slot 指向有效的 AtomicU64
                unsafe { &*self.read_slot }.store(actual_read + 1, Ordering::Release);
                // SAFETY: slot < capacity，self.data + slot 在数据区域范围内
                return Some(unsafe { &*self.data.add(slot) });
            }

            if start.elapsed() >= timeout {
                return None;
            }
            std::hint::spin_loop();
        }
    }

    /// 读取端落后消息数。
    pub fn lag(&self) -> u64 {
        // SAFETY: self.header 在 connect 时验证，指向有效的 Header
        let header = unsafe { &*self.header };
        let write_seq = header.write_seq.load(Ordering::Acquire);
        // SAFETY: self.read_slot 在 connect 时分配，指向有效的 AtomicU64
        let read_seq = unsafe { &*self.read_slot }.load(Ordering::Acquire);
        write_seq.saturating_sub(read_seq)
    }
}

impl<T> Drop for Reader<T> {
    fn drop(&mut self) {
        if !self.read_slot.is_null() {
            // SAFETY: read_slot 在 connect 时分配，指向共享内存中的有效 AtomicU64
            unsafe { &*self.read_slot }.store(FREE_READER, Ordering::Release);
        }
        let _ = unmap_shared(self.map, self.map_len);
        close_fd(self.fd);
    }
}

// SAFETY: Reader 可以安全地在线程间转移所有权，因为：
// 1. 共享内存映射在所有线程中具有相同的地址
// 2. 每个 Reader 拥有独立的 read_slot，不与其他 Reader 共享
// 3. 对共享内存的访问通过原子操作保证线程安全
unsafe impl<T: Send> Send for Reader<T> {}

pub fn cleanup(addr: &Address) -> Result<(), Error> {
    shm_unlink(addr, true).or_else(|_| shm_unlink(addr, false))
}
