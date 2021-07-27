FROM ubuntu:20.04

ARG OASIS_CORE_NODE_BINARY
ARG OASIS_CORE_RUNTUME_LOADER

RUN apt-get -y update && apt-get install -y bubblewrap

COPY ${OASIS_CORE_NODE_BINARY} /oasis/bin/oasis-node
COPY ${OASIS_CORE_RUNTUME_LOADER} /oasis/bin/oasis-core-runtime-loader
COPY tests/benchmark/benchmark /oasis/bin/benchmark
COPY target/release/test-runtime-benchmarking /oasis/lib/oasis-runtime

ENV PATH "/oasis/bin:${PATH}"
