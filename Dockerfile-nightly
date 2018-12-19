FROM rustlang/rust:nightly

RUN apt-get update && \
    apt-get install -y cmake && \
    rm -rf /var/lib/apt/lists/*

COPY . /opt/tarpaulin/

RUN cd /opt/tarpaulin/ && \
    RUSTFLAGS="--cfg procmacro2_semver_exempt" cargo install --path . && \
    rm -rf /opt/tarpaulin/ && \
    rm -rf /usr/local/cargo/registry/

WORKDIR /volume

CMD cargo build && cargo tarpaulin
