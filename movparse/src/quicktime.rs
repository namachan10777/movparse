//! Parser implementation for Apple QuickTime format based on [apple document](https://developer.apple.com/library/archive/documentation/QuickTime/QTFF/QTFFChap2/qtff2.html)
use std::{io, time::Duration};

use movparse_box::{AttrRead, BoxHeader, BoxRead, RawString, Reader, U32Tag};
use movparse_derive::{BoxRead, RootRead};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncSeek};

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "ftyp")]
pub struct Ftyp {
    #[mp4(header)]
    pub header: BoxHeader,
    pub major_brand: U32Tag,
    pub minor_version: U32Tag,
    pub compatible_brands: Vec<U32Tag>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mdat {
    header: BoxHeader,
    pos: u64,
}

impl Mdat {
    pub async fn read_exact<R: AsyncRead + AsyncSeek + Unpin + Send>(
        &self,
        reader: &mut Reader<R>,
        offset: u64,
        buf: &mut [u8],
    ) -> io::Result<()> {
        reader.seek_from_start(self.pos + offset).await?;
        reader.read_exact(buf).await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl BoxRead for Mdat {
    fn acceptable_tag(tag: [u8; 4]) -> bool {
        tag == [b'm', b'd', b'a', b't']
    }

    async fn read_body<R: AsyncRead + AsyncSeek + Unpin + Send>(
        header: BoxHeader,
        reader: &mut Reader<R>,
    ) -> Result<Self, io::Error> {
        let pos = reader.pos;
        reader.seek_from_current(header.body_size() as i64).await?;
        Ok(Self { header, pos })
    }
}

#[derive(Clone, RootRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickTime {
    pub ftyp: Ftyp,
    pub moov: Moov,
    pub mdat: Mdat,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timescale(u32);

#[async_trait::async_trait]
impl AttrRead for Timescale {
    async fn read_attr<R: AsyncRead + AsyncSeek + Unpin + Send>(
        reader: &mut Reader<R>,
    ) -> io::Result<Self> {
        Ok(Self(u32::read_attr(reader).await?))
    }
}

impl Timescale {
    pub fn decode_duration(&self, dur: u32) -> Duration {
        Duration::from_secs(dur as u64) / self.0
    }
}

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    pub time_scale: Timescale,
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

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edit {
    pub track_duration: u32,
    pub media_time: u32,
    pub media_rate: u32,
}

#[async_trait::async_trait]
impl AttrRead for Edit {
    async fn read_attr<R: AsyncRead + AsyncSeek + Unpin + Send>(
        reader: &mut Reader<R>,
    ) -> Result<Self, io::Error> {
        let track_duration = u32::read_attr(reader).await?;
        let media_time = u32::read_attr(reader).await?;
        let media_rate = u32::read_attr(reader).await?;
        Ok(Self {
            track_duration,
            media_rate,
            media_time,
        })
    }
}

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "elst")]
pub struct Elst {
    #[mp4(header)]
    pub header: BoxHeader,
    pub version: u8,
    pub flags: [u8; 3],
    pub number_of_entries: u32,
    pub edit_list: Vec<Edit>,
}

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "internal")]
#[mp4(tag = "edts")]
pub struct Edts {
    #[mp4(header)]
    pub header: BoxHeader,
    pub edit_list: Elst,
}

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "mdhd")]
pub struct Mdhd {
    #[mp4(header)]
    pub header: BoxHeader,
    pub version: u8,
    pub flags: [u8; 3],
    pub creation_time: u32,
    pub modification_time: u32,
    pub time_scale: Timescale,
    pub duration: u32,
    pub language: u16,
    pub quality: u16,
}

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "internal")]
#[mp4(tag = "minf")]
pub struct Minf {
    #[mp4(header)]
    pub header: BoxHeader,
    pub dinf: Dinf,
    pub stbl: Stbl,
}

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "internal")]
#[mp4(tag = "mdia")]
pub struct Mdia {
    #[mp4(header)]
    pub header: BoxHeader,
    pub mdhd: Mdhd,
    pub hdlr: Hdlr,
    pub minf: Minf,
}

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "internal")]
#[mp4(tag = "trak")]
pub struct Trak {
    #[mp4(header)]
    pub header: BoxHeader,
    pub tkhd: Tkhd,
    pub edts: Edts,
    pub mdia: Mdia,
}

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "internal")]
#[mp4(tag = "udta")]
pub struct Udta {
    #[mp4(header)]
    pub header: BoxHeader,
}

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "url ")]
pub struct DataReference {
    #[mp4(header)]
    pub header: BoxHeader,
    pub version: u8,
    pub flags: [u8; 3],
    pub data: Vec<u8>,
}

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "internal")]
#[mp4(tag = "dinf")]
pub struct Dinf {
    #[mp4(header)]
    pub header: BoxHeader,
    pub dref: Dref,
}

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "stco")]
pub struct Stco {
    #[mp4(header)]
    pub header: BoxHeader,
    pub version: u8,
    pub flags: [u8; 3],
    pub number_of_entries: u32,
    pub chunk_offset_table: Vec<u32>,
}

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "co64")]
pub struct Co64 {
    #[mp4(header)]
    pub header: BoxHeader,
    pub version: u8,
    pub flags: [u8; 3],
    pub number_of_entries: u32,
    pub chunk_offset_table: Vec<u64>,
}

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "sgpd")]
pub struct Sgpd {
    #[mp4(header)]
    pub header: BoxHeader,
}

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "sbgp")]
pub struct Sbgp {
    #[mp4(header)]
    pub header: BoxHeader,
}

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "internal")]
#[mp4(tag = "stbl")]
pub struct Stbl {
    #[mp4(header)]
    pub header: BoxHeader,
    pub stsd: Stsd,
    pub stts: Stts,
    pub stsc: Stsc,
    pub stsz: Stsz,
    pub stco: Option<Stco>,
    pub co64: Option<Co64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Sample {
    pub duration: Duration,
    pub offset: usize,
    pub size: usize,
}

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, BoxRead, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[mp4(boxtype = "internal")]
#[mp4(tag = "moov")]
pub struct Moov {
    #[mp4(header)]
    pub header: BoxHeader,
    pub mvhd: Mvhd,
    pub traks: Vec<Trak>,
    pub udta: Udta,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackMetadata {
    pub samples: Vec<Sample>,
}

impl From<&Stco> for Co64 {
    fn from(stco: &Stco) -> Self {
        Self {
            header: stco.header,
            version: stco.version,
            flags: stco.flags,
            number_of_entries: stco.number_of_entries,
            chunk_offset_table: stco
                .chunk_offset_table
                .iter()
                .map(|offset| *offset as u64)
                .collect(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SamplesError {
    #[error("co64 or stco not found")]
    Co64OrStcoNotFound,
}

impl Trak {
    pub fn samples(&self) -> Result<Vec<Sample>, SamplesError> {
        let timescale = &self.mdia.mdhd.time_scale;
        let sample_to_chunk_table = &self.mdia.minf.stbl.stsc.sample_to_chunk_table;
        let co64 = match &self.mdia.minf.stbl.stco {
            Some(stco) => Some(stco.into()),
            None => self.mdia.minf.stbl.co64.clone(),
        }
        .ok_or(SamplesError::Co64OrStcoNotFound)?;
        let chunk_offset_table = co64.chunk_offset_table;
        let _ = &self.mdia.minf.stbl.stsd.sample_description_table;
        let sample_size_table = &self.mdia.minf.stbl.stsz.sample_size_table;
        let time_to_sample_table = &self.mdia.minf.stbl.stts.time_to_sample_table;
        let mut samples = Vec::new();
        let sample_len = time_to_sample_table
            .iter()
            .fold(0, |sample_len, time_to_sample| {
                time_to_sample.sample_count + sample_len
            }) as usize;
        samples.resize(
            sample_len,
            Sample {
                duration: Duration::from_secs(0),
                offset: 0,
                size: 0,
            },
        );
        // set sample durations
        let mut sample_idx = 0;
        for time_to_sample in time_to_sample_table {
            for _ in 0..(time_to_sample.sample_count as usize) {
                samples[sample_idx].duration =
                    timescale.decode_duration(time_to_sample.sample_duration);
                sample_idx += 1;
            }
        }
        // set chunk offset per samples
        let mut sample_idx = 0;
        for (sample_to_chunk_idx, sample_to_chunk) in sample_to_chunk_table.iter().enumerate() {
            let first_chunk_idx = sample_to_chunk.first_chunk as usize - 1;
            let next_chunk_idx = sample_to_chunk_table
                .get(sample_to_chunk_idx + 1)
                .map(|sample_to_chunk| sample_to_chunk.first_chunk as usize - 1)
                .unwrap_or_else(|| chunk_offset_table.len());
            for chunk_offset in &chunk_offset_table[first_chunk_idx..next_chunk_idx] {
                let mut offset_in_chunk = 0;
                for _ in 0..sample_to_chunk.samples_per_chunk {
                    println!(
                        "sample: {} offset: {}, offset_in_chunk: {}",
                        sample_idx, chunk_offset, offset_in_chunk
                    );
                    samples[sample_idx].offset = offset_in_chunk + *chunk_offset as usize;
                    samples[sample_idx].size = sample_size_table[sample_idx] as usize;
                    offset_in_chunk += sample_size_table[sample_idx] as usize;
                    sample_idx += 1;
                }
            }
        }
        Ok(samples)
    }
}

impl Moov {
    pub fn video_duration(&self) -> Duration {
        self.mvhd.time_scale.decode_duration(self.mvhd.duration)
    }
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
