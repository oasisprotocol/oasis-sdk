#!/bin/sh -eux
. ./consts.sh

docker run --rm "$ENVOY_DOCKER_IMAGE" --help
