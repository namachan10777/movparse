VERSION 0.6

FROM rust:1.65.0-slim-bullseye
RUN apt-get update && apt-get install -y wget
RUN rustup component add rustfmt clippy
RUN cargo install cargo-chef
WORKDIR /work
RUN wget https://download.samplelib.com/mp4/sample-5s.mp4

recipe:
    COPY Cargo.* .
    COPY movparse/src movparse/src
    COPY movparse/Cargo.* movparse/
    COPY movparse-box/src movparse-box/src
    COPY movparse-box/tests movparse-box/tests
    COPY movparse-box/Cargo.* movparse-box/
    COPY movparse-derive/src movparse-derive/src
    COPY movparse-derive/tests movparse-derive/tests
    COPY movparse-derive/Cargo.* movparse-derive/
    COPY README.md .
    RUN cargo chef prepare --recipe-path recipe.json
    SAVE ARTIFACT recipe.json /recipe.json

prepare-debug:
    COPY +recipe/recipe.json .
    RUN cargo chef cook --recipe-path recipe.json
    RUN cargo check
    COPY Cargo.* .
    COPY movparse/src movparse/src
    COPY movparse/Cargo.* movparse/
    COPY movparse-box/src movparse-box/src
    COPY movparse-box/tests movparse-box/tests
    COPY movparse-box/Cargo.* movparse-box/
    COPY movparse-derive/src movparse-derive/src
    COPY movparse-derive/tests movparse-derive/tests
    COPY movparse-derive/Cargo.* movparse-derive/
    COPY README.md .

prepare-release:
    COPY +recipe/recipe.json .
    RUN cargo chef cook --recipe-path recipe.json
    RUN cargo doc
    COPY Cargo.* .
    COPY movparse/src movparse/src
    COPY movparse/Cargo.* movparse/
    COPY movparse-box/src movparse-box/src
    COPY movparse-box/tests movparse-box/tests
    COPY movparse-box/Cargo.* movparse-box/
    COPY movparse-derive/src movparse-derive/src
    COPY movparse-derive/tests movparse-derive/tests
    COPY movparse-derive/Cargo.* movparse-derive/
    COPY README.md .
    RUN cargo doc

test:
    FROM +prepare-debug
    RUN cargo test
    RUN cargo clippy -- -D warnings
    RUN cargo fmt -- --check