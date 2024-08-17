#! /usr/bin/env bash

targets=(aarch64-apple-darwin
         aarch64-pc-windows-msvc
         aarch64-unknown-linux-gnu
         aarch64-unknown-linux-musl
         x86_64-apple-darwin
         x86_64-pc-windows-msvc
         x86_64-unknown-linux-gnu
         x86_64-unknown-linux-musl)

cargo install cross --git https://github.com/cross-rs/cross

for target in "${targets[@]}"; do
    echo "Building for $target"
    cross build --release --target "$target"
done