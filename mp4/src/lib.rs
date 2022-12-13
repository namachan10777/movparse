use mp4_box::{BoxHeader, U32Tag};
use mp4_derive::{BoxRead, RootRead};

#[derive(BoxRead, Debug, PartialEq, Eq)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "ftyp")]
pub struct Ftyp {
    #[mp4(header)]
    pub header: BoxHeader,
    pub major_brand: U32Tag,
    pub minor_version: U32Tag,
    pub compatible_brands: Vec<U32Tag>,
}

#[derive(RootRead, Debug, PartialEq, Eq)]
pub struct QuickTime {
    pub ftyp: Ftyp,
    pub moov: Moov,
}

#[derive(BoxRead, Debug, PartialEq, Eq)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "mvhd")]
pub struct Mvhd {
    #[mp4(header)]
    pub header: BoxHeader,
    pub version: u8,
    pub flags: [u8; 3],
    pub creation_time: u32,
    pub modification_time: u32,
    pub time_scale: u32,
    pub duration: u32,
    pub preferred_rate: u32,
    pub preferred_volume: u16,
    pub _reserved: [u8; 10],
    pub matrix_structure: [u8; 36],
    pub preview_time: u32,
    pub preview_duration: u32,
    pub poster_time: u32,
    pub selection_time: u32,
    pub selection_duration: u32,
    pub current_time: u32,
    pub next_track_id: u32,
}

#[derive(BoxRead, Debug, PartialEq, Eq)]
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
    pub matrix_structure: [u8; 36],
    pub track_width: u32,
    pub track_height: u32,
}

#[derive(BoxRead, Debug, PartialEq, Eq)]
#[mp4(boxtype = "internal")]
#[mp4(tag = "edts")]
pub struct Edts {
    #[mp4(header)]
    header: BoxHeader,
}

#[derive(BoxRead, Debug, PartialEq, Eq)]
#[mp4(boxtype = "internal")]
#[mp4(tag = "trak")]
pub struct Trak {
    #[mp4(header)]
    pub header: BoxHeader,
    pub tkhd: Tkhd,
    pub edts: Edts,
}

#[derive(BoxRead, Debug, PartialEq, Eq)]
#[mp4(boxtype = "internal")]
#[mp4(tag = "udta")]
pub struct Udta {
    #[mp4(header)]
    header: BoxHeader,
}

#[derive(BoxRead, Debug, PartialEq, Eq)]
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
    use mp4_box::{Reader, RootRead};
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
        panic!("{:#?}", quicktime);
    }
}
