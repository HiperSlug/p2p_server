#!/usr/bin/env bash
set -e

PROTOC_ZIP=protoc-25.3-linux-x86_64.zip

curl -OL https://github.com/protocolbuffers/protobuf/releases/download/v25.3/$PROTOC_ZIP
unzip -o $PROTOC_ZIP -d $HOME/.protoc
export PATH="$HOME/.protoc/bin:$PATH"

cargo build --release
