FROM rust:slim as builder

RUN apt-get update && \
    apt-get install -y libssl-dev pkg-config cmake zlib1g-dev && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /opt/tarpaulin

RUN env USER=root cargo init .

COPY Cargo.toml .
COPY Cargo.lock .
RUN mkdir .cargo
RUN cargo vendor > .cargo/config

COPY src /opt/tarpaulin/src

RUN cd /opt/tarpaulin/ && \
    cargo install --locked --path . && \
    rm -rf /opt/tarpaulin/ && \
    rm -rf /usr/local/cargo/registry/

FROM rust:slim

COPY --from=builder /usr/local/cargo/bin/cargo-tarpaulin /usr/local/cargo/bin/cargo-tarpaulin

WORKDIR /volume

CMD cargo build && cargo tarpaulin
