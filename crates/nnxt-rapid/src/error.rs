//! 错误类型定义。

use std::fmt;

#[derive(Debug)]
pub enum Error {
    AddressInUse,
    NotFound,
    InvalidHeader,
    SizeMismatch { expected: usize, actual: usize },
    CapacityTooSmall,
    ReaderSlotsFull,
    ShmOpenFailed { errno: i32 },
    ShmUnlinkFailed { errno: i32 },
    FileOpenFailed { errno: i32 },
    FileUnlinkFailed { errno: i32 },
    FileStatFailed { errno: i32 },
    MmapFailed { errno: i32 },
    MunmapFailed { errno: i32 },
    TruncateFailed { errno: i32 },
    Timeout,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::AddressInUse => write!(f, "address already in use"),
            Error::NotFound => write!(f, "shared memory not found"),
            Error::InvalidHeader => write!(f, "invalid shared memory header"),
            Error::SizeMismatch { expected, actual } => {
                write!(f, "shared memory size mismatch expected=[{}] actual=[{}]", expected, actual)
            }
            Error::CapacityTooSmall => write!(f, "capacity too small"),
            Error::ReaderSlotsFull => write!(f, "no available reader slot"),
            Error::ShmOpenFailed { errno } => write!(f, "shm_open failed errno=[{}]", errno),
            Error::ShmUnlinkFailed { errno } => write!(f, "shm_unlink failed errno=[{}]", errno),
            Error::FileOpenFailed { errno } => write!(f, "file open failed errno=[{}]", errno),
            Error::FileUnlinkFailed { errno } => write!(f, "file unlink failed errno=[{}]", errno),
            Error::FileStatFailed { errno } => write!(f, "fstat failed errno=[{}]", errno),
            Error::MmapFailed { errno } => write!(f, "mmap failed errno=[{}]", errno),
            Error::MunmapFailed { errno } => write!(f, "munmap failed errno=[{}]", errno),
            Error::TruncateFailed { errno } => write!(f, "ftruncate failed errno=[{}]", errno),
            Error::Timeout => write!(f, "timeout while reading"),
        }
    }
}

impl std::error::Error for Error {}
