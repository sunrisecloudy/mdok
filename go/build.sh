#!/bin/zsh
# Build the Go mdok port binaries.
set -e
cd "$(dirname "$0")"
mkdir -p bin
go build -o bin/mdok ./cmd/mdok
go build -o bin/test-server ./cmd/test-server
echo "built bin/mdok and bin/test-server"
