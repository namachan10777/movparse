use mp4_box::BoxHeader;
use mp4_derive::{BoxRead, RootRead};

#[derive(BoxRead, Debug, PartialEq, Eq)]
#[mp4(boxtype = "leaf")]
#[mp4(tag = "ftyp")]
struct Ftyp {
    #[mp4(header)]
    pub header: BoxHeader,
    pub major_brand: [u8; 4],
    pub minor_version: [u8; 4],
    pub compatible_brands: Vec<[u8; 4]>,
}

#[derive(RootRead, Debug, PartialEq, Eq)]
struct QuickTime {
    ftyp: Ftyp,
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
        let file = fs::File::open("../cat.mov").await.unwrap();
        let limit = file.metadata().await.unwrap().len() as u64;
        let mut reader = Reader::new(file, limit);
        let quicktime = QuickTime::read(&mut reader).await.unwrap();
        let major = String::from_utf8_lossy(&quicktime.ftyp.major_brand);
        let minor = String::from_utf8_lossy(&quicktime.ftyp.minor_version);
        let compatibles = quicktime
            .ftyp
            .compatible_brands
            .iter()
            .map(|brand| String::from_utf8_lossy(brand).to_string())
            .collect::<Vec<_>>();
        assert_eq!(major, "qt  ");
        assert_eq!(minor, "\0\0\u{2}\0");
        assert_eq!(compatibles, vec!["qt  ".to_owned()]);
    }
}
