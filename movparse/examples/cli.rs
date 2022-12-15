use std::path::PathBuf;

use clap::Parser;
use movparse::{Reader, RootRead};
use tokio::fs;

#[derive(Parser)]
struct Opts {
    file: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();
    let file = fs::File::open(opts.file).await?;
    let limit = file.metadata().await?.len();
    let mut reader = Reader::new(file, limit);
    let mp4 = movparse::quicktime::QuickTime::read(&mut reader).await?;
    let json = serde_json::to_string_pretty(&mp4)?;
    println!("{}", json);
    Ok(())
}
