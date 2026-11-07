use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ReadError {
    #[error("packet ended at {at}, needed {need} more bytes")]
    Short { at: usize, need: usize },
    #[error("invalid UTF-8")]
    Utf8,
    #[error("invalid JSON: {0}")]
    Json(String),
    #[error("unknown opcode {0}")]
    Opcode(u8),
}

pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

macro_rules! read_num {
    ($name:ident, $t:ty) => {
        pub fn $name(&mut self) -> Result<$t, ReadError> {
            let bytes = self.take(std::mem::size_of::<$t>())?;
            let arr = bytes.try_into().map_err(|_| ReadError::Short {
                at: self.pos,
                need: std::mem::size_of::<$t>(),
            })?;
            Ok(<$t>::from_le_bytes(arr))
        }
    };
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8], ReadError> {
        if self.remaining() < n {
            return Err(ReadError::Short {
                at: self.pos,
                need: n - self.remaining(),
            });
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    pub fn skip(&mut self, n: usize) -> Result<(), ReadError> {
        self.take(n).map(|_| ())
    }

    pub fn u8(&mut self) -> Result<u8, ReadError> {
        Ok(self.take(1)?[0])
    }

    read_num!(u16, u16);
    read_num!(u32, u32);
    read_num!(i32, i32);
    read_num!(f32, f32);
    read_num!(f64, f64);

    pub fn utf8z(&mut self) -> Result<&'a str, ReadError> {
        let rest = &self.data[self.pos..];
        let end = rest.iter().position(|&b| b == 0).ok_or(ReadError::Short {
            at: self.data.len(),
            need: 1,
        })?;
        self.pos += end + 1;
        std::str::from_utf8(&rest[..end]).map_err(|_| ReadError::Utf8)
    }
}

#[derive(Default)]
pub struct Writer {
    buf: Vec<u8>,
}

macro_rules! write_num {
    ($name:ident, $t:ty) => {
        pub fn $name(&mut self, v: $t) -> &mut Self {
            self.buf.extend_from_slice(&v.to_le_bytes());
            self
        }
    };
}

impl Writer {
    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.buf.push(v);
        self
    }

    write_num!(u16, u16);
    write_num!(u32, u32);
    write_num!(i32, i32);
    write_num!(f32, f32);
    write_num!(f64, f64);

    pub fn bytes(&mut self, b: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(b);
        self
    }

    pub fn utf8z(&mut self, s: &str) -> &mut Self {
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(0);
        self
    }

    pub fn finish(self) -> Vec<u8> {
        self.buf
    }
}
