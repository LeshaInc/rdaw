use std::borrow::Cow;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    None = 0,
    Zstd = 1,
}

impl Compression {
    pub fn from_u8(v: u8) -> Option<Compression> {
        match v {
            0 => Some(Compression::None),
            1 => Some(Compression::Zstd),
            _ => None,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn compress<'a>(&self, data: &'a [u8]) -> std::io::Result<Cow<'a, [u8]>> {
        match self {
            Compression::None => Ok(data.into()),
            Compression::Zstd => Ok(zstd::bulk::compress(data, 0)?.into()),
        }
    }

    pub fn decompress<'a>(
        &self,
        uncompressed_len: usize,
        data: &'a [u8],
    ) -> std::io::Result<Cow<'a, [u8]>> {
        match self {
            Compression::None => Ok(data.into()),
            Compression::Zstd => Ok(zstd::bulk::decompress(data, uncompressed_len)?.into()),
        }
    }
}
