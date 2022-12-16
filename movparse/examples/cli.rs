use std::path::PathBuf;

use clap::Parser;
use movparse::{Reader, RootRead};
use tokio::fs;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
struct Opts {
    file: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();
    let opts = Opts::parse();
    let file = fs::File::open(opts.file).await?;
    let limit = file.metadata().await?.len();
    let mut reader = Reader::new(file, limit);
    let mp4 = movparse::quicktime::QuickTime::read(&mut reader).await?;
    for (idx, sample) in mp4.moov.traks[0].samples().iter().enumerate() {
        let mut buf = Vec::new();
        buf.resize(sample.size, 0);
        reader.seek_from_start(sample.offset as u64).await?;
        reader.read_exact(&mut buf).await?;
        println!("buf: {}: {} from: {}", idx, buf.len(), sample.offset);
    }
    Ok(())
}
