//! rapid: 共享内存 SPMC 广播式环形缓冲区。
//!
//! 提供基于路径的队列寻址与零拷贝读写接口。设计目标是最小化延迟与
//! 运行时开销，适用于行情广播、交易订单等高频数据流。

mod address;
mod error;
mod ring;

pub use address::{Address, AddressError};
pub use error::Error;
pub use ring::{cleanup, Reader, Writer};

pub const DEFAULT_MAX_READERS: usize = 16;
