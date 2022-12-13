VERSION 0.6

FROM rust:1.65.0-slim-bullseye
RUN apt-get update && apt-get install -y wget
RUN rustup component add rustfmt clippy
RUN cargo install cargo-chef
WORKDIR /work
RUN wget https://download.samplelib.com/mp4/sample-5s.mp4

recipe:
    COPY Cargo.* .
    COPY mp4/src mp4/src
    COPY mp4/Cargo.* mp4/
    COPY mp4-box/src mp4-box/src
    COPY mp4-box/tests mp4-box/tests
    COPY mp4-box/Cargo.* mp4-box/
    COPY mp4-derive/src mp4-derive/src
    COPY mp4-derive/tests mp4-derive/tests
    COPY mp4-derive/Cargo.* mp4-derive/
    RUN cargo chef prepare --recipe-path recipe.json
    SAVE ARTIFACT recipe.json /recipe.json

prepare-debug:
    COPY +recipe/recipe.json .
    RUN cargo chef cook --recipe-path recipe.json
    RUN cargo check
    COPY Cargo.* .
    COPY mp4/src mp4/src
    COPY mp4/Cargo.* mp4/
    COPY mp4-box/src mp4-box/src
    COPY mp4-box/tests mp4-box/tests
    COPY mp4-box/Cargo.* mp4-box/
    COPY mp4-derive/src mp4-derive/src
    COPY mp4-derive/tests mp4-derive/tests
    COPY mp4-derive/Cargo.* mp4-derive/

prepare-release:
    COPY +recipe/recipe.json .
    RUN cargo chef cook --recipe-path recipe.json
    COPY Cargo.* .
    COPY mp4/src mp4/src
    COPY mp4/Cargo.* mp4/
    COPY mp4-box/src mp4-box/src
    COPY mp4-box/tests mp4-box/tests
    COPY mp4-box/Cargo.* mp4-box/
    COPY mp4-derive/src mp4-derive/src
    COPY mp4-derive/tests mp4-derive/tests
    COPY mp4-derive/Cargo.* mp4-derive/

test:
    FROM +prepare-debug
    RUN cargo test
    RUN cargo clippy -- -D warnings
    RUN cargo fmt -- --check