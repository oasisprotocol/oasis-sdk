#!/bin/sh -eux
. ./consts.sh

../../../../tests/download-artifacts.sh

if [ ! -e "untracked/simple-keymanager" ]; then
    curl -fLo "untracked/simple-keymanager" "$SIMPLE_KEYMANAGER_ARTIFACT"
    chmod +x "untracked/simple-keymanager"
fi
