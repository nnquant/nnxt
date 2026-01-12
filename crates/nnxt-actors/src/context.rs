//! ActorContext: 封装队列通信能力。

use std::collections::HashMap;

use crossbeam_channel::Sender;
use nng::options::{Options, RecvTimeout};
use nng::Socket;
use nnxt_rapid::{cleanup, Address, Reader, Writer};

use crate::error::Error;
use crate::reactor::{ControlHandle, RapidSource, RapidSourcesHandle};

type WriterEntry = Box<dyn AnyWriter>;

/// 类型擦除的 Writer trait
pub trait AnyWriter: Send {
    fn prepare_raw(&mut self) -> *mut u8;
    fn commit(&mut self);
}

/// 类型化 Writer 包装
struct TypedWriter<T: Copy + Send + 'static> {
    inner: Writer<T>,
}

impl<T: Copy + Send + 'static> AnyWriter for TypedWriter<T> {
    fn prepare_raw(&mut self) -> *mut u8 {
        self.inner.prepare() as *mut T as *mut u8
    }
    fn commit(&mut self) {
        self.inner.commit();
    }
}

/// Actor 上下文，提供队列读写能力
pub struct ActorContext {
    rapid_sources: RapidSourcesHandle,
    writers: HashMap<String, WriterEntry>,
    control_handle: ControlHandle,
    external_tx: Sender<Box<dyn std::any::Any + Send>>,
}

impl ActorContext {
    pub fn new(
        rapid_sources: RapidSourcesHandle,
        control_handle: ControlHandle,
        external_tx: Sender<Box<dyn std::any::Any + Send>>,
    ) -> Self {
        Self {
            rapid_sources,
            writers: HashMap::new(),
            control_handle,
            external_tx,
        }
    }

    /// 连接到已存在的队列进行读取
    pub fn read_from<M: Copy + Send + 'static>(&mut self, addr: &str) -> Result<usize, Error> {
        let address = Address::new(addr).map_err(|_| Error::QueueNotFound(addr.to_string()))?;
        let reader = Reader::<M>::connect(&address)?;
        let mut sources = self.rapid_sources.borrow_mut();
        let source_id = sources.len();
        sources.push(RapidSource::new(source_id, reader));
        Ok(source_id)
    }

    /// 创建队列用于写入
    pub fn write_to<M: Copy + Send + 'static>(
        &mut self,
        addr: &str,
        capacity: usize,
    ) -> Result<(), Error> {
        if self.writers.contains_key(addr) {
            return Err(Error::QueueAlreadyExists(addr.to_string()));
        }
        let address = Address::new(addr).map_err(|_| Error::QueueNotFound(addr.to_string()))?;
        let _ = cleanup(&address);
        let writer = Writer::<M>::create(&address, capacity)?;
        self.writers.insert(
            addr.to_string(),
            Box::new(TypedWriter { inner: writer }),
        );
        Ok(())
    }

    /// 发布消息到队列
    pub fn publish<M: Copy>(&mut self, addr: &str, msg: &M) -> Result<(), Error> {
        let writer = self
            .writers
            .get_mut(addr)
            .ok_or_else(|| Error::QueueNotFound(addr.to_string()))?;
        // SAFETY: 调用者保证类型匹配
        unsafe {
            let ptr = writer.prepare_raw() as *mut M;
            std::ptr::write(ptr, *msg);
        }
        writer.commit();
        Ok(())
    }

    /// 回复 control 消息
    pub fn reply_control(&mut self, response: &[u8]) -> Result<(), Error> {
        let mut handle = self.control_handle.borrow_mut();
        let Some(socket) = handle.as_mut() else {
            return Err(Error::ControlNotAvailable);
        };
        if let Err((_msg, err)) = socket.send(response) {
            return Err(Error::NngError(err));
        }
        Ok(())
    }

    /// 设置 control socket，用于接收控制消息。
    pub fn set_control_socket(&mut self, socket: Socket) -> Result<(), Error> {
        socket.set_opt::<RecvTimeout>(Some(std::time::Duration::from_millis(0)))?;
        *self.control_handle.borrow_mut() = Some(socket);
        Ok(())
    }

    /// 获取外部事件发送器，用于外部线程推送事件。
    pub fn external_sender(&self) -> Sender<Box<dyn std::any::Any + Send>> {
        self.external_tx.clone()
    }

}
