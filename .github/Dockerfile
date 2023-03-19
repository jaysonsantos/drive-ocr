FROM rust:1-bookworm
RUN apt update && apt install -y protobuf-compiler ocrmypdf
RUN curl -Lo zig.tar.xz \
    https://ziglang.org/builds/zig-linux-$(uname -m)-0.11.0-dev.2160+49d37e2d1.tar.xz && \
    mkdir /opt/zig && \
    tar xf zig.tar.xz -C /opt/zig --strip-components=1 && \
    rm zig.tar.xz

RUN curl -Lo fd.deb https://github.com/sharkdp/fd/releases/download/v8.7.0/fd_8.7.0_$(dpkg --print-architecture).deb &&\
    dpkg -i fd.deb

ENV PATH="/opt/zig:${PATH}"

RUN rustup toolchain install -t x86_64-unknown-linux-musl,aarch64-unknown-linux-musl -c clippy,rustfmt nightly
RUN rustup default nightly
RUN curl -L https://github.com/rust-cross/cargo-zigbuild/releases/download/v0.16.3/cargo-zigbuild-v0.16.3.$(uname -m)-unknown-linux-musl.tar.gz | \
    tar xvf - -C /usr/local/bin/
