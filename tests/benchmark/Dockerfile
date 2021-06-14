FROM ubuntu:20.04

ARG OASIS_CORE_VERSION

RUN apt-get -y update && apt-get install -y bubblewrap

COPY tests/untracked/oasis_core_${OASIS_CORE_VERSION}_linux_amd64/oasis-node /oasis/bin/oasis-node
COPY tests/untracked/oasis_core_${OASIS_CORE_VERSION}_linux_amd64/oasis-core-runtime-loader /oasis/bin/oasis-core-runtime-loader
COPY tests/benchmark/benchmark /oasis/bin/benchmark
COPY target/debug/test-runtime-benchmarking /oasis/lib/oasis-runtime

ENV PATH "/oasis/bin:${PATH}"
