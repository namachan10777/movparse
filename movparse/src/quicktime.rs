use std::io;

use movparse_box::{AttrRead, BoxHeader, RawString, Reader, U32Tag};
use movparse_derive::{BoxRead, RootRead};
use tokio::io::{AsyncRead, AsyncSeek};
use serde::{Serialize, Deserialize};

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "ftyp")]
pub struct Ftyp {
    #[mp4(header)]
    pub header: BoxHeader,
    pub major_brand: U32Tag,
    pub minor_version: U32Tag,
    pub compatible_brands: Vec<U32Tag>,
}

#[derive(RootRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickTime {
    pub ftyp: Ftyp,
    pub moov: Moov,
}

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "mvhd")]
pub struct Mvhd {
    #[mp4(header)]
    pub header: BoxHeader,
    pub version: u8,
    #[serde(with = "movparse_box::util::serde::u8_array")]
    pub flags: [u8; 3],
    pub creation_time: u32,
    pub modification_time: u32,
    pub time_scale: u32,
    pub duration: u32,
    pub preferred_rate: u32,
    pub preferred_volume: u16,
    #[serde(with = "movparse_box::util::serde::u8_array")]
    pub _reserved: [u8; 10],
    #[serde(with = "movparse_box::util::serde::u8_array")]
    pub matrix_structure: [u8; 36],
    pub preview_time: u32,
    pub preview_duration: u32,
    pub poster_time: u32,
    pub selection_time: u32,
    pub selection_duration: u32,
    pub current_time: u32,
    pub next_track_id: u32,
}

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "tkhd")]
pub struct Tkhd {
    #[mp4(header)]
    pub header: BoxHeader,
    pub version: u8,
    pub flags: [u8; 3],
    pub creation_time: u32,
    pub modification_time: u32,
    pub trak_id: u32,
    _reserved: [u8; 4],
    pub duration: u32,
    _reserved2: [u8; 8],
    pub layer: u16,
    pub alternate_group: u16,
    pub volume: u16,
    _reserved3: [u8; 2],
    #[serde(with = "movparse_box::util::serde::u8_array")]
    pub matrix_structure: [u8; 36],
    pub track_width: u32,
    pub track_height: u32,
}

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "internal")]
#[mp4(tag = "edts")]
pub struct Edts {
    #[mp4(header)]
    pub header: BoxHeader,
}

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "mdhd")]
pub struct Mdhd {
    #[mp4(header)]
    pub header: BoxHeader,
    pub version: u8,
    pub flags: [u8; 3],
    pub creation_time: u32,
    pub modification_time: u32,
    pub time_scale: u32,
    pub duration: u32,
    pub language: u16,
    pub quality: u16,
}

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "internal")]
#[mp4(tag = "minf")]
pub struct Minf {
    #[mp4(header)]
    pub header: BoxHeader,
    pub dinf: Dinf,
    pub stbl: Stbl,
}

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "internal")]
#[mp4(tag = "mdia")]
pub struct Mdia {
    #[mp4(header)]
    pub header: BoxHeader,
    pub mdhd: Mdhd,
    pub hdlr: Hdlr,
    pub minf: Minf,
}

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "internal")]
#[mp4(tag = "trak")]
pub struct Trak {
    #[mp4(header)]
    pub header: BoxHeader,
    pub tkhd: Tkhd,
    pub edts: Edts,
    pub mdia: Mdia,
}

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "hdlr")]
pub struct Hdlr {
    #[mp4(header)]
    pub header: BoxHeader,
    pub version: u8,
    pub flags: [u8; 3],
    pub component_type: U32Tag,
    pub component_subtype: U32Tag,
    pub component_flags: [u8; 4],
    pub component_flags_mask: [u8; 4],
    pub component_name: RawString,
}

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "internal")]
#[mp4(tag = "udta")]
pub struct Udta {
    #[mp4(header)]
    pub header: BoxHeader,
}

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "url ")]
pub struct DataReference {
    #[mp4(header)]
    pub header: BoxHeader,
    pub version: u8,
    pub flags: [u8; 3],
    pub data: Vec<u8>,
}

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "dref")]
pub struct Dref {
    #[mp4(header)]
    pub header: BoxHeader,
    pub version: u8,
    pub flags: [u8; 3],
    pub number_of_entries: u32,
    pub drefs: Vec<DataReference>,
}

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "internal")]
#[mp4(tag = "dinf")]
pub struct Dinf {
    #[mp4(header)]
    pub header: BoxHeader,
    pub dref: Dref,
}

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
pub enum GeneralSampleDescription {
    #[mp4(tag = "mp4a")]
    Mp4a {
        #[mp4(header)]
        header: BoxHeader,
        _reserved: [u8; 6],
        data_reference_index: u16,
    },
    #[mp4(tag = "avc1")]
    Avc1 {
        #[mp4(header)]
        header: BoxHeader,
        _reserved: [u8; 6],
        data_reference_index: u16,
        version: u16,
        revision: u16,
        vendor: U32Tag,
        temporal_quality: u32,
        spatial_quality: u32,
        width: u16,
        height: u16,
        horizontal_resolution: u32,
        vertical_resolution: u32,
        data_size: u32,
        frame_per_samples: u16,
    },
    #[mp4(tag = "Hap1")]
    Hap1 {
        #[mp4(header)]
        header: BoxHeader,
        _reserved: [u8; 6],
        data_reference_index: u16,
        version: u16,
        revision: u16,
        vendor: U32Tag,
        temporal_quality: u32,
        spatial_quality: u32,
        width: u16,
        height: u16,
        horizontal_resolution: u32,
        vertical_resolution: u32,
        data_size: u32,
        frame_per_samples: u16,
    },
}

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "stsd")]
pub struct Stsd {
    #[mp4(header)]
    pub header: BoxHeader,
    pub version: u8,
    pub flags: [u8; 3],
    pub number_of_entries: u32,
    pub sample_description_table: Vec<GeneralSampleDescription>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeToSample {
    pub sample_count: u32,
    pub sample_duration: u32,
}

#[async_trait::async_trait]
impl AttrRead for TimeToSample {
    async fn read_attr<R: AsyncRead + AsyncSeek + Unpin + Send>(
        reader: &mut Reader<R>,
    ) -> Result<Self, io::Error> {
        let sample_count = AttrRead::read_attr(reader).await?;
        let sample_duration = AttrRead::read_attr(reader).await?;
        Ok(Self {
            sample_count,
            sample_duration,
        })
    }
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SampleToChunk {
    pub first_chunk: u32,
    pub samples_per_chunk: u32,
    pub sample_description_id: u32,
}

#[async_trait::async_trait]
impl AttrRead for SampleToChunk {
    async fn read_attr<R: AsyncRead + AsyncSeek + Unpin + Send>(
        reader: &mut Reader<R>,
    ) -> Result<Self, io::Error> {
        let first_chunk = AttrRead::read_attr(reader).await?;
        let samples_per_chunk = AttrRead::read_attr(reader).await?;
        let sample_description_id = AttrRead::read_attr(reader).await?;
        Ok(Self {
            first_chunk,
            samples_per_chunk,
            sample_description_id,
        })
    }
}

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "stts")]
pub struct Stts {
    #[mp4(header)]
    pub header: BoxHeader,
    pub version: u8,
    pub flags: [u8; 3],
    pub number_of_entries: u32,
    pub time_to_sample_table: Vec<TimeToSample>,
}

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "stsc")]
pub struct Stsc {
    #[mp4(header)]
    pub header: BoxHeader,
    pub version: u8,
    pub flags: [u8; 3],
    pub number_of_entries: u32,
    pub sample_to_chunk_table: Vec<SampleToChunk>,
}

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "stsz")]
pub struct Stsz {
    #[mp4(header)]
    pub header: BoxHeader,
    pub version: u8,
    pub flags: [u8; 3],
    pub sample_size: u32,
    pub number_of_entries: u32,
    pub sample_size_table: Vec<u32>,
}

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "stco")]
pub struct Stco {
    #[mp4(header)]
    pub header: BoxHeader,
    pub version: u8,
    pub flags: [u8; 3],
    pub number_of_entries: u32,
    pub sample_offset_table: Vec<u32>,
}

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "sgpd")]
pub struct Sgpd {
    #[mp4(header)]
    pub header: BoxHeader,
}

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "sbgp")]
pub struct Sbgp {
    #[mp4(header)]
    pub header: BoxHeader,
}

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "internal")]
#[mp4(tag = "stbl")]
pub struct Stbl {
    #[mp4(header)]
    pub header: BoxHeader,
    pub stsd: Stsd,
    pub stts: Stts,
    pub stsc: Stsc,
    pub stsz: Stsz,
    pub stco: Stco,
}

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "smhd")]
pub struct Smhd {
    #[mp4(header)]
    pub header: BoxHeader,
    pub version: u8,
    pub flags: [u8; 3],
    pub balance: u16,
    _reserved: u16,
}

#[derive(BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "internal")]
#[mp4(tag = "moov")]
pub struct Moov {
    #[mp4(header)]
    pub header: BoxHeader,
    pub mvhd: Mvhd,
    pub traks: Vec<Trak>,
    pub udta: Udta,
}

#[cfg(test)]
mod test {
    use movparse_box::{Reader, RootRead};
    use tokio::fs;
    use tracing_subscriber::{
        prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, EnvFilter,
    };

    use super::*;
    #[tokio::test]
    async fn test_real_file() {
        tracing_subscriber::registry()
            .with(EnvFilter::from_default_env())
            .with(tracing_subscriber::fmt::layer())
            .init();
        let file = fs::File::open("../sample-5s.mp4").await.unwrap();
        let limit = file.metadata().await.unwrap().len() as u64;
        let mut reader = Reader::new(file, limit);
        let quicktime = QuickTime::read(&mut reader).await.unwrap();
        let major = String::from_utf8_lossy(&quicktime.ftyp.major_brand.raw);
        let minor = String::from_utf8_lossy(&quicktime.ftyp.minor_version.raw);
        let compatibles = quicktime
            .ftyp
            .compatible_brands
            .iter()
            .map(|brand| String::from_utf8_lossy(&brand.raw).to_string())
            .collect::<Vec<_>>();
        assert_eq!(major, "isom");
        assert_eq!(minor, "\0\0\u{2}\0");
        assert_eq!(
            compatibles,
            vec![
                "isom".to_owned(),
                "iso2".to_owned(),
                "avc1".to_owned(),
                "mp41".to_owned()
            ]
        );
    }
}
