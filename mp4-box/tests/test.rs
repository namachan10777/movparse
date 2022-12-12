use mp4_box::*;
use tokio::io::{self, AsyncRead, AsyncSeek};

#[derive(Debug, PartialEq, Eq)]
pub struct Ftyp {
    pub header: BoxHeader,
    pub major_brand: [u8; 4],
    pub minor_version: [u8; 4],
    pub compatible_brands: Vec<[u8; 4]>,
}

#[async_trait::async_trait]
impl mp4_box::BoxRead for Ftyp {
    fn acceptable_tag(tag: [u8; 4]) -> bool {
        tag == [b'f', b't', b'y', b'p']
    }
    async fn read<R: AsyncRead + AsyncSeek + Unpin + Send>(
        header: BoxHeader,
        reader: &mut Reader<R>,
    ) -> io::Result<Self> {
        let mut reader2 = reader.clone();
        reader2.set_limit(header.body_size() as u64);
        let major_brand = AttrRead::read(&mut reader2).await?;
        let minor_version = AttrRead::read(&mut reader2).await?;
        let compatible_brands = AttrRead::read(&mut reader2).await?;
        reader.seek_from_current(header.body_size() as i64).await?;
        Ok(Self {
            header,
            major_brand,
            minor_version,
            compatible_brands,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Data {
    header: BoxHeader,
}

#[async_trait::async_trait]
impl mp4_box::BoxRead for Data {
    fn acceptable_tag(tag: [u8; 4]) -> bool {
        tag == [b'd', b'a', b't', b'a']
    }
    async fn read<R: AsyncRead + AsyncSeek + Unpin + Send>(
        header: BoxHeader,
        reader: &mut Reader<R>,
    ) -> io::Result<Self> {
        reader.seek_from_current(header.body_size() as i64).await?;
        Ok(Self { header })
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Test {
    header: BoxHeader,
    ftyp: Ftyp,
    data: Vec<Data>,
}

#[async_trait::async_trait]
impl mp4_box::BoxRead for Test {
    fn acceptable_tag(tag: [u8; 4]) -> bool {
        tag == [b't', b'e', b's', b't']
    }
    async fn read<R: AsyncRead + AsyncSeek + Unpin + Send>(
        header: BoxHeader,
        reader: &mut Reader<R>,
    ) -> io::Result<Self> {
        let mut reader2 = reader.clone();
        reader2.set_limit(header.body_size() as u64);
        let mut ftyp = Ftyp::placeholder();
        let mut data = Vec::<Data>::placeholder();
        while reader2.remain() > 0 {
            let header = BoxHeader::read(&mut reader2).await?;
            if ftyp.acceptable_tag(header.id) {
                let ftyp_value = ftyp.read(header, &mut reader2).await?;
                BoxPlaceholder::push(&mut ftyp, ftyp_value)?;
            } else if data.acceptable_tag(header.id) {
                let data_value = data.read(header, &mut reader2).await?;
                BoxPlaceholder::push(&mut data, data_value)?;
            }
        }
        reader.seek_from_current(header.body_size() as i64).await?;
        Ok(Self {
            header,
            ftyp: ftyp.get()?,
            data: data.get()?,
        })
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use tokio::io::AsyncWriteExt;

    use super::*;

    #[tokio::test]
    async fn test() {
        let mut ftyp = Vec::new();
        let major_brand = &[b'r', b'u', b's', b't'];
        let minor_version = &[b'm', b'p', b'4', b'r'];
        let mut compatible_brands = Vec::new();
        compatible_brands
            .write_all(&[b'f', b'o', b'o', b'0'])
            .await
            .unwrap();
        compatible_brands
            .write_all(&[b'h', b'o', b'g', b'e'])
            .await
            .unwrap();
        ftyp.write_u32(
            4 + 4
                + major_brand.len() as u32
                + minor_version.len() as u32
                + compatible_brands.len() as u32,
        )
        .await
        .unwrap();
        ftyp.write_all(&[b'f', b't', b'y', b'p']).await.unwrap();
        ftyp.write_all(major_brand).await.unwrap();
        ftyp.write_all(minor_version).await.unwrap();
        ftyp.write_all(&compatible_brands).await.unwrap();

        let ftyp_src = Cursor::new(ftyp.clone());
        let mut ftyp_reader = mp4_box::Reader::new(ftyp_src, ftyp.len() as u64);
        let ftyp_header = mp4_box::BoxHeader::read(&mut ftyp_reader).await.unwrap();
        ftyp_reader.set_limit(ftyp_header.body_size() as u64);
        let ftyp_body: Ftyp = mp4_box::BoxRead::read(ftyp_header, &mut ftyp_reader)
            .await
            .unwrap();
        assert_eq!(
            ftyp_body,
            Ftyp {
                header: BoxHeader {
                    id: [b'f', b't', b'y', b'p'],
                    size: 24
                },
                major_brand: [b'r', b'u', b's', b't'],
                minor_version: [b'm', b'p', b'4', b'r'],
                compatible_brands: vec![[b'f', b'o', b'o', b'0'], [b'h', b'o', b'g', b'e'],]
            }
        );

        let mut data1 = Vec::new();
        data1.write_u32(108).await.unwrap();
        data1.write_all(&[b'd', b'a', b't', b'a']).await.unwrap();
        data1.write_all(&[255u8; 100]).await.unwrap();

        let mut data2 = Vec::new();
        data2.write_u32(108).await.unwrap();
        data2.write_all(&[b'd', b'a', b't', b'a']).await.unwrap();
        data2.write_all(&[254u8; 100]).await.unwrap();

        let mut test = Vec::new();
        test.write_u32(ftyp.len() as u32 + data1.len() as u32 + data2.len() as u32 + 8)
            .await
            .unwrap();
        test.write_all(&[b't', b'e', b's', b't']).await.unwrap();
        test.write_all(&data1).await.unwrap();
        test.write_all(&ftyp).await.unwrap();
        test.write_all(&data2).await.unwrap();

        let test_src = Cursor::new(test.clone());
        let mut reader = Reader::new(test_src, test.len() as u64);
        let test_header = BoxHeader::read(&mut reader).await.unwrap();
        let test_body = Test::read(test_header, &mut reader).await.unwrap();
        assert_eq!(
            test_body,
            Test {
                header: BoxHeader {
                    id: [b't', b'e', b's', b't'],
                    size: 108 + 108 + 24 + 8
                },
                ftyp: ftyp_body,
                data: vec![
                    Data {
                        header: BoxHeader {
                            id: [b'd', b'a', b't', b'a'],
                            size: 108
                        },
                    },
                    Data {
                        header: BoxHeader {
                            id: [b'd', b'a', b't', b'a'],
                            size: 108
                        },
                    }
                ]
            }
        );
    }
}
