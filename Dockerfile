FROM rust

RUN apt-get update && apt-get install -y cmake

COPY . /opt/tarpaulin/

RUN cd /opt/tarpaulin/ && \
    cargo install && \
    rm -rf /opt/tarpaulin/ && \
    rm -rf /usr/local/cargo/registry/

WORKDIR /volume

CMD cargo build && cargo tarpaulin
