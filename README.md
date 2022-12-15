# movparse

[![CI](https://github.com/namachan10777/movparse/actions/workflows/ci.yml/badge.svg)](https://github.com/namachan10777/movparse/actions/workflows/ci.yml)

Derive-macro base async-aware mp4 family (`quicktime`, `mp4` and more) parser.
*This library is under development*

```rust, no_run
use std::path::PathBuf;
use clap::Parser;
use movparse::{Reader, RootRead, BoxRead, U32Tag, BoxHeader};
use tokio::fs;

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
}

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
    let mp4 = QuickTime::read(&mut reader).await?;
    println!("{:#?}", mp4);
    Ok(())
}
```
