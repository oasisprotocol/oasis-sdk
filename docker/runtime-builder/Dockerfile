FROM ubuntu:20.04

# Package versions.
ARG GO_VERSION=1.16.4
ARG RUST_NIGHTLY_VERSION=2021-05-07
ARG OASIS_CORE_VERSION=21.2.7

ARG DEBIAN_FRONTEND=noninteractive

RUN apt-get update -qq && apt-get upgrade -qq && apt-get install -qq \
    git gcc curl jq pkg-config libssl-dev protobuf-compiler && \
    apt-get autoclean && apt-get autoremove && rm -rf /var/cache/apt/archives/* && \
    mkdir -p /src /cargo/.rustup

ENV HOME="/root"
ENV GOPATH="/go"
ENV CARGO_HOME="/cargo"
ENV RUSTUP_HOME="/cargo/.rustup"
ENV PATH="${CARGO_HOME}/bin:${GOPATH}/bin:/usr/local/go/bin:${PATH}"
ENV SDK_SRC="/src"

# Install Rust.
RUN curl "https://sh.rustup.rs" -sfo rustup.sh && \
    sh rustup.sh -y --default-toolchain nightly-${RUST_NIGHTLY_VERSION} && \
    rustup target add x86_64-fortanix-unknown-sgx && \
    cargo install --version 0.4.0 fortanix-sgx-tools && \
    cargo install --version 0.8.2 sgxs-tools && \
    rm -rf /cargo/registry /rustup.sh

# Install Go and utilities.
RUN curl -OL https://dl.google.com/go/go${GO_VERSION}.linux-amd64.tar.gz && \
    tar -C /usr/local -xzf go${GO_VERSION}.linux-amd64.tar.gz && \
    rm go${GO_VERSION}.linux-amd64.tar.gz && \
    mkdir -p /go/bin

# Install extra tools from oasis-core.
RUN cd /usr/local/src && \
    curl -OL https://github.com/oasisprotocol/oasis-core/archive/refs/tags/v${OASIS_CORE_VERSION}.tar.gz && \
    tar -xzf v${OASIS_CORE_VERSION}.tar.gz && cd oasis-core-${OASIS_CORE_VERSION} && \
    cargo install --force --path tools && \
    cd .. && rm -rf v${OASIS_CORE_VERSION}.tar.gz oasis-core-${OASIS_CORE_VERSION} && rm -rf /cargo/registry
