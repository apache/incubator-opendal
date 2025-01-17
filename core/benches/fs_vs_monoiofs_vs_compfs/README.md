# OpenDAL services fs vs. monoiofs vs. compfs

This benchmark compares the performance of OpenDAL services fs, monoiofs and compfs.

## Goal

We expect monoiofs and compfs to outperform tokio-based fs in speed.

## Usage

For test: `cargo run`

For bench: `cargo run --release -- --bench`
