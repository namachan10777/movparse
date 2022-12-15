use std::{
    fmt::Debug,
    io::{self, Cursor},
    sync::Arc,
};

pub mod util;

use byteorder::{ReadBytesExt, BE};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, SeekFrom},
    sync::Mutex,
};

#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct BoxHeader {
    pub id: [u8; 4],
    pub size: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct U32Tag {
    pub raw: [u8; 4],
}

#[derive(Clone, PartialEq, Eq)]
pub struct RawString {
    raw: Vec<u8>,
    str: String,
}

#[cfg(feature = "serde")]
impl serde::Serialize for RawString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.str)
    }
}

#[cfg(feature = "serde")]
struct RawStringVisitor;

#[cfg(feature = "serde")]
impl<'de> serde::de::Visitor<'de> for RawStringVisitor {
    type Value = RawString;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("RawString accept only string type")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(RawString {
            str: v.to_owned(),
            raw: v.as_bytes().to_vec(),
        })
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RawString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(RawStringVisitor)
    }
}

impl std::fmt::Debug for RawString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.str.fmt(f)
    }
}

impl std::fmt::Debug for U32Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let utf8 = String::from_utf8_lossy(&self.raw);
        f.write_fmt(format_args!(
            "<\"{}\": 0x{:02x}{:02x}{:02x}{:02x}>",
            utf8, self.raw[3], self.raw[2], self.raw[1], self.raw[0]
        ))
    }
}

impl BoxHeader {
    pub async fn read<R: AsyncRead + AsyncSeek + Unpin + Send>(
        reader: &mut Reader<R>,
    ) -> io::Result<BoxHeader> {
        let mut id = [0u8; 4];
        let mut size = [0u8; 4];

        reader.read_exact(&mut size).await?;
        reader.read_exact(&mut id).await?;

        let size = ReadBytesExt::read_u32::<BE>(&mut Cursor::new(size)).unwrap();
        if size < 8 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{:?}: size must be larger than 8", U32Tag { raw: id }),
            ));
        }
        #[cfg(feature = "tracing")]
        {
            let id_str_repr = String::from_utf8_lossy(&id);
            tracing::debug!(
                "<{:?}: 0x{}{}{}{}> with size {} bytes",
                id_str_repr,
                id[3],
                id[2],
                id[1],
                id[0],
                size
            );
        }
        Ok(Self { id, size })
    }

    pub fn body_size(&self) -> usize {
        (self.size - 8) as usize
    }
}

impl Debug for BoxHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoxHeader")
            .field("id", &U32Tag { raw: self.id })
            .field("size", &self.size)
            .finish()
    }
}

pub struct Reader<R: AsyncRead + AsyncSeek + Unpin + Send> {
    inner: Arc<Mutex<R>>,
    pos: u64,
    limit: Option<u64>,
}

impl<R: AsyncRead + AsyncSeek + Unpin + Send> Clone for Reader<R> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            pos: self.pos,
            limit: self.limit,
        }
    }
}

impl<R: AsyncRead + Unpin + Send + AsyncSeek> Reader<R> {
    pub fn new(reader: R, limit: u64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(reader)),
            pos: 0,
            limit: Some(limit),
        }
    }

    pub async fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut inner = self.inner.lock().await;
        inner.seek(SeekFrom::Start(self.pos)).await?;
        let size = inner.read_exact(buf).await?;

        if let Some(limit) = self.limit {
            #[cfg(feature = "tracing")]
            {
                tracing::trace!(
                    "read {} bytes from {}. next position: {}, limit: {}",
                    size,
                    self.pos,
                    self.pos + size as u64,
                    limit
                );
            }
            if self.pos + size as u64 > limit {
                return Err(io::Error::new(
                    io::ErrorKind::OutOfMemory,
                    format!("pos: {} over the limit {}", self.pos + size as u64, limit),
                ));
            }
        } else {
            #[cfg(feature = "tracing")]
            {
                tracing::trace!(
                    "read {} bytes from {}. next position: {}",
                    size,
                    self.pos - size as u64,
                    self.pos
                );
            }
        }
        self.pos += size as u64;
        Ok(size)
    }

    pub fn set_limit(&mut self, limit: u64) {
        self.limit = Some(limit + self.pos);
    }

    pub async fn seek_from_current(&mut self, seek: i64) -> io::Result<()> {
        #[cfg(feature = "tracing")]
        {
            tracing::trace!("seek {} to {}", self.pos, self.pos as i64 + seek,);
        }
        self.inner
            .lock()
            .await
            .seek(SeekFrom::Current(seek as i64))
            .await?;
        self.pos = (self.pos as i64 + seek) as u64;
        Ok(())
    }

    pub async fn seek_from_start(&mut self, seek: u64) -> io::Result<()> {
        #[cfg(feature = "tracing")]
        {
            tracing::trace!("seek {} to {}", self.pos, seek,);
        }
        self.inner.lock().await.seek(SeekFrom::Start(seek)).await?;
        self.pos = seek;
        Ok(())
    }

    pub fn clear_limit(&mut self) {
        self.limit = None;
    }

    pub fn remain(&self) -> i64 {
        self.limit
            .map(|limit| limit as i64 - self.pos as i64)
            .unwrap_or(i64::MAX)
    }
}

#[async_trait::async_trait]
pub trait AttrRead: Sized {
    async fn read_attr<R: AsyncRead + AsyncSeek + Unpin + Send>(
        reader: &mut Reader<R>,
    ) -> io::Result<Self>;
}

#[async_trait::async_trait]
impl AttrRead for RawString {
    async fn read_attr<R: AsyncRead + AsyncSeek + Unpin + Send>(
        reader: &mut Reader<R>,
    ) -> io::Result<Self> {
        let mut buf = Vec::new();
        buf.resize(reader.remain() as usize, 0);
        reader.read_exact(&mut buf).await?;
        let str = String::from_utf8_lossy(&buf).to_string();
        Ok(Self { raw: buf, str })
    }
}

#[async_trait::async_trait]
impl<T: BoxRead> AttrRead for T {
    async fn read_attr<R: AsyncRead + AsyncSeek + Unpin + Send>(
        reader: &mut Reader<R>,
    ) -> io::Result<Self> {
        let header = BoxHeader::read(reader).await?;
        if Self::acceptable_tag(header.id) {
            BoxRead::read_body(header, reader).await
        } else {
            let u32tag = U32Tag { raw: header.id };
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{:?} is not acceptable for me", u32tag),
            ))
        }
    }
}

#[async_trait::async_trait]
pub trait BoxRead: Sized {
    fn acceptable_tag(tag: [u8; 4]) -> bool;
    async fn read_body<R: AsyncRead + AsyncSeek + Unpin + Send>(
        header: BoxHeader,
        reader: &mut Reader<R>,
    ) -> io::Result<Self>;
}

#[async_trait::async_trait]
pub trait RootRead: Sized {
    async fn read<R: AsyncRead + AsyncSeek + Unpin + Send>(
        reader: &mut Reader<R>,
    ) -> io::Result<Self>;
}

#[async_trait::async_trait]
impl<const N: usize> AttrRead for [u8; N] {
    async fn read_attr<R: AsyncRead + AsyncSeek + Unpin + Send>(
        reader: &mut Reader<R>,
    ) -> io::Result<Self> {
        let mut buf = [0u8; N];
        reader.read_exact(&mut buf[..]).await?;
        Ok(buf)
    }
}

#[async_trait::async_trait]
impl AttrRead for u8 {
    async fn read_attr<R: AsyncRead + AsyncSeek + Unpin + Send>(
        reader: &mut Reader<R>,
    ) -> io::Result<Self> {
        let mut buf = [0u8; 1];
        reader.read_exact(&mut buf[..]).await?;
        Ok(buf[0])
    }
}

#[async_trait::async_trait]
impl AttrRead for u16 {
    async fn read_attr<R: AsyncRead + AsyncSeek + Unpin + Send>(
        reader: &mut Reader<R>,
    ) -> io::Result<Self> {
        let mut buf = [0u8; 2];
        reader.read_exact(&mut buf[..]).await?;
        let mut buf = io::Cursor::new(buf);
        Ok(ReadBytesExt::read_u16::<BE>(&mut buf).unwrap())
    }
}

#[async_trait::async_trait]
impl AttrRead for u32 {
    async fn read_attr<R: AsyncRead + AsyncSeek + Unpin + Send>(
        reader: &mut Reader<R>,
    ) -> io::Result<Self> {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf[..]).await?;
        let mut buf = io::Cursor::new(buf);
        Ok(ReadBytesExt::read_u32::<BE>(&mut buf).unwrap())
    }
}

#[async_trait::async_trait]
impl<T: AttrRead + Send> AttrRead for Vec<T> {
    async fn read_attr<R: AsyncRead + AsyncSeek + Unpin + Send>(
        reader: &mut Reader<R>,
    ) -> io::Result<Self> {
        let mut buf = Vec::new();
        loop {
            match T::read_attr(reader).await {
                Ok(t) => {
                    buf.push(t);
                    if reader.remain() == 0 {
                        return Ok(buf);
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::OutOfMemory => {
                    return Ok(buf);
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl AttrRead for U32Tag {
    async fn read_attr<R: AsyncRead + AsyncSeek + Unpin + Send>(
        reader: &mut Reader<R>,
    ) -> io::Result<Self> {
        let raw: [u8; 4] = AttrRead::read_attr(reader).await?;
        Ok(Self { raw })
    }
}

#[async_trait::async_trait]
pub trait BoxPlaceholder<T: BoxRead> {
    type Output;
    fn push(&mut self, value: T) -> io::Result<()>;
    fn get(self, name: &str) -> io::Result<Self::Output>;
    fn acceptable_tag(&self, tag: [u8; 4]) -> bool;
    async fn read_body<R: AsyncRead + AsyncSeek + Unpin + Send>(
        &self,
        header: BoxHeader,
        reader: &mut Reader<R>,
    ) -> io::Result<T>;
}

pub trait BoxContainer<T: BoxRead + Sync, D: BoxPlaceholder<T>> {
    fn placeholder() -> D;
}

pub struct SingleBoxPlaceholder<T> {
    inner: Option<T>,
}

#[async_trait::async_trait]
impl<T: BoxRead + Sync> BoxPlaceholder<T> for SingleBoxPlaceholder<T> {
    type Output = T;

    fn acceptable_tag(&self, tag: [u8; 4]) -> bool {
        T::acceptable_tag(tag)
    }

    async fn read_body<R: AsyncRead + AsyncSeek + Unpin + Send>(
        &self,
        header: BoxHeader,
        reader: &mut Reader<R>,
    ) -> io::Result<T> {
        T::read_body(header, reader).await
    }

    fn push(&mut self, value: T) -> io::Result<()> {
        if self.inner.is_some() {
            return Err(io::Error::new(io::ErrorKind::AddrInUse, "already inserted"));
        }
        self.inner = Some(value);
        Ok(())
    }

    fn get(self, name: &str) -> io::Result<Self::Output> {
        match self.inner {
            Some(inner) => Ok(inner),
            None => Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("field {} was not inserted", name),
            )),
        }
    }
}

#[async_trait::async_trait]
impl<T: BoxRead + Sync> BoxPlaceholder<T> for Option<T> {
    type Output = Option<T>;

    fn acceptable_tag(&self, tag: [u8; 4]) -> bool {
        T::acceptable_tag(tag)
    }

    async fn read_body<R: AsyncRead + AsyncSeek + Unpin + Send>(
        &self,
        header: BoxHeader,
        reader: &mut Reader<R>,
    ) -> io::Result<T> {
        T::read_body(header, reader).await
    }
    fn push(&mut self, value: T) -> io::Result<()> {
        *self = Some(value);
        Ok(())
    }
    fn get(self, _: &str) -> io::Result<Self::Output> {
        Ok(self)
    }
}

#[async_trait::async_trait]
impl<T: BoxRead + Sync> BoxPlaceholder<T> for Vec<T> {
    type Output = Vec<T>;

    fn acceptable_tag(&self, tag: [u8; 4]) -> bool {
        T::acceptable_tag(tag)
    }

    async fn read_body<R: AsyncRead + AsyncSeek + Unpin + Send>(
        &self,
        header: BoxHeader,
        reader: &mut Reader<R>,
    ) -> io::Result<T> {
        T::read_body(header, reader).await
    }
    fn push(&mut self, value: T) -> io::Result<()> {
        self.push(value);
        Ok(())
    }
    fn get(self, _: &str) -> io::Result<Self::Output> {
        Ok(self)
    }
}

impl<T: BoxRead + Sync> BoxContainer<T, SingleBoxPlaceholder<T>> for T {
    fn placeholder() -> SingleBoxPlaceholder<T> {
        SingleBoxPlaceholder { inner: None }
    }
}

impl<T: BoxRead + Sync> BoxContainer<T, Option<T>> for Option<T> {
    fn placeholder() -> Option<T> {
        None
    }
}

impl<T: BoxRead + Sync> BoxContainer<T, Vec<T>> for Vec<T> {
    fn placeholder() -> Vec<T> {
        Vec::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io;

    use crate::AttrRead;

    #[tokio::test]
    async fn test_read_u8_array() {
        let src = vec![0u8, 1, 2, 3, 4];
        let limit = src.len() as u64;
        let reader = io::Cursor::new(src);
        let mut reader = Reader::new(reader, limit);
        let target: [u8; 3] = AttrRead::read_attr(&mut reader).await.unwrap();
        assert_eq!(target, [0, 1, 2]);
        assert_eq!(reader.pos, 3);
        assert_eq!(reader.limit, Some(5));
        let target: [u8; 2] = AttrRead::read_attr(&mut reader).await.unwrap();
        assert_eq!(target, [3, 4]);
        assert_eq!(reader.pos, 5);
        assert_eq!(reader.limit, Some(5));
    }

    #[tokio::test]
    async fn test_read_limit() {
        let src = vec![0u8, 1, 2, 3, 4];
        let limit = src.len() as u64;
        let reader = io::Cursor::new(src);
        let mut reader = Reader::new(reader, limit);
        reader.set_limit(2);
        assert!(<[u8; 3]>::read_attr(&mut reader).await.is_err());
        assert_eq!(<[u8; 2]>::read_attr(&mut reader).await.unwrap(), [0, 1]);
        reader.clear_limit();
        assert_eq!(<[u8; 2]>::read_attr(&mut reader).await.unwrap(), [2, 3]);
    }

    #[tokio::test]
    async fn test_seek() {
        let src = vec![0u8, 1, 2, 3, 4];
        let limit = src.len() as u64;
        let reader = io::Cursor::new(src);
        let mut reader = Reader::new(reader, limit);
        assert_eq!(<[u8; 2]>::read_attr(&mut reader).await.unwrap(), [0, 1]);
        assert_eq!(<[u8; 2]>::read_attr(&mut reader).await.unwrap(), [2, 3]);
    }

    #[tokio::test]
    async fn test_shared_reader() {
        let src = vec![0u8, 1, 2, 3, 4];
        let limit = src.len() as u64;
        let reader = io::Cursor::new(src);
        let mut reader = Reader::new(reader, limit);
        assert_eq!(<[u8; 2]>::read_attr(&mut reader).await.unwrap(), [0, 1]);
        let mut reader2 = reader.clone();
        assert_eq!(<[u8; 2]>::read_attr(&mut reader).await.unwrap(), [2, 3]);
        assert_eq!(<[u8; 3]>::read_attr(&mut reader2).await.unwrap(), [2, 3, 4]);
    }

    #[tokio::test]
    async fn test_vec() {
        let src = vec![0u8, 1, 2, 3, 4];
        let limit = src.len() as u64;
        let reader = io::Cursor::new(src);
        let mut reader = Reader::new(reader, limit);
        reader.set_limit(4);
        assert_eq!(
            Vec::<[u8; 2]>::read_attr(&mut reader).await.unwrap(),
            vec![[0, 1], [2, 3]]
        );
    }
}
