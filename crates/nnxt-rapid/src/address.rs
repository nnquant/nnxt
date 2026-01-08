//! 队列地址解析与共享内存路径映射。

use std::fmt;
use std::path::{Path, PathBuf};

const SHM_ROOT: &str = "/dev/shm/nnxt";
const SHM_PREFIX: &str = "/nnxt";

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Address {
    path: String,
    shm_name: String,
    file_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AddressError {
    Empty,
    EmptySegment,
    InvalidChar { ch: char },
    InvalidSegment { segment: String },
}

impl fmt::Display for AddressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AddressError::Empty => write!(f, "address path is empty"),
            AddressError::EmptySegment => write!(f, "address path has empty segment"),
            AddressError::InvalidChar { ch } => write!(f, "address path has invalid char [{}]", ch),
            AddressError::InvalidSegment { segment } => {
                write!(f, "address path has invalid segment [{}]", segment)
            }
        }
    }
}

impl std::error::Error for AddressError {}

impl Address {
    /// 解析路径并生成共享内存地址。
    ///
    /// # Arguments
    /// * `path` - 斜杠分隔的逻辑路径（如 "market/ctp"）
    ///
    /// # Returns
    /// Address 实例
    ///
    /// # Errors
    /// 当路径为空或包含非法字符时返回 AddressError
    ///
    /// # Example
    /// ```
    /// use nnxt_rapid::Address;
    /// let addr = Address::new("market/ctp").unwrap();
    /// assert_eq!(addr.path(), "market/ctp");
    /// ```
    pub fn new(path: &str) -> Result<Self, AddressError> {
        if path.is_empty() {
            return Err(AddressError::Empty);
        }
        if path.starts_with('/') {
            return Err(AddressError::InvalidChar { ch: '/' });
        }
        let mut segments = Vec::new();
        for seg in path.split('/') {
            if seg.is_empty() {
                return Err(AddressError::EmptySegment);
            }
            if seg == "." || seg == ".." {
                return Err(AddressError::InvalidSegment {
                    segment: seg.to_string(),
                });
            }
            for ch in seg.chars() {
                if !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_') {
                    return Err(AddressError::InvalidChar { ch });
                }
            }
            segments.push(seg);
        }
        let normalized = segments.join("/");
        let shm_name = format!("{}/{}", SHM_PREFIX, normalized);
        let file_path = Path::new(SHM_ROOT).join(&normalized);
        Ok(Self {
            path: normalized,
            shm_name,
            file_path,
        })
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn shm_name(&self) -> &str {
        &self.shm_name
    }

    pub fn file_path(&self) -> &Path {
        &self.file_path
    }
}
