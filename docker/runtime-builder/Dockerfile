FROM ubuntu:20.04

# Package versions.
ARG GO_VERSION=1.18.5
ARG RUST_NIGHTLY_VERSION
ARG OASIS_CORE_COMMIT
ARG FORTANIX_TOOLS_VERSION=0.5.1
ARG SGXS_TOOLS_VERSION=0.8.6

ARG DEBIAN_FRONTEND=noninteractive

RUN apt-get update -qq && apt-get upgrade -qq && apt-get install -qq \
    git gcc curl jq pkg-config libssl-dev protobuf-compiler libclang-dev clang-11 gcc-multilib cmake && \
    apt-get autoclean && apt-get autoremove && rm -rf /var/cache/apt/archives/* && \
    mkdir -p /src /cargo/.rustup

ENV HOME="/root"
ENV GOPATH="/go"
ENV CARGO_HOME="/cargo"
ENV RUSTUP_HOME="/cargo/.rustup"
ENV PATH="${CARGO_HOME}/bin:${GOPATH}/bin:/usr/local/go/bin:${PATH}"

# Install Rust.
RUN curl "https://sh.rustup.rs" -sfo rustup.sh && \
    sh rustup.sh -y --default-toolchain nightly-${RUST_NIGHTLY_VERSION} && \
    rustup target add x86_64-fortanix-unknown-sgx && \
    cargo install --version ${FORTANIX_TOOLS_VERSION} fortanix-sgx-tools && \
    cargo install --version ${SGXS_TOOLS_VERSION} sgxs-tools && \
    rm -rf /cargo/registry /rustup.sh

# Install Go and utilities.
RUN curl -OL https://dl.google.com/go/go${GO_VERSION}.linux-amd64.tar.gz && \
    tar -C /usr/local -xzf go${GO_VERSION}.linux-amd64.tar.gz && \
    rm go${GO_VERSION}.linux-amd64.tar.gz && \
    mkdir -p /go/bin

# Install extra tools from oasis-core.
RUN cd /usr/local/src && \
    curl -OL https://github.com/oasisprotocol/oasis-core/archive/${OASIS_CORE_COMMIT}.tar.gz && \
    tar -xzf ${OASIS_CORE_COMMIT}.tar.gz && cd oasis-core-* && \
    cargo install --locked --force --path tools && \
    cd .. && rm -rf ${OASIS_CORE_COMMIT}.tar.gz oasis-core-* && rm -rf /cargo/registry

VOLUME /src
