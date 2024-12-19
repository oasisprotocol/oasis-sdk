FROM ghcr.io/oasisprotocol/oasis-core-dev:stable-24.3.x AS oasis-core-dev

ARG OASIS_CLI_VERSION=0.10.3

ENV RUSTFLAGS="-C target-feature=+aes,+ssse3"
ENV RUSTDOCFLAGS="-C target-feature=+aes,+ssse3"

RUN curl -L -o /tmp/cli.tar.gz "https://github.com/oasisprotocol/cli/releases/download/v${OASIS_CLI_VERSION}/oasis_cli_${OASIS_CLI_VERSION}_linux_amd64.tar.gz" && \
    tar -C /usr/bin -xf /tmp/cli.tar.gz --strip-components 1 "oasis_cli_${OASIS_CLI_VERSION}_linux_amd64/oasis" && \
    rm /tmp/cli.tar.gz

VOLUME /src

WORKDIR /src
