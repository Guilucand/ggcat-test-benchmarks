# GGCAT helper repository

This repository is a collection of tools and data used for the benchmarks with GGCAT against other tools.
The benchmarks results are available in the preprint article [Extremely fast construction and querying of compacted and colored de Bruijn graphs with GGCAT](https://doi.org/10.1101/2022.10.24.513174)

## Benchmarks setup

To download and compile all the tools used in the benchmarks run the script `setup-bench.sh`.

## Benchmarking tool

The tool used for benchmarking is a custom program written in Rust.

The \*.sh files in the project directory are scripts used to invoke the benchmarks with various configurations with/without slurm.

Run `cargo run --release -- --help` to get a list of available options for the benchmarking tool, and refer to the various scripts for examples on how to run the benchmarks.

## Benchmarking config

There are three files for the benchmarks configuration, under the folder config/:

`benchmarks.toml` contains the description of the various benchmarks performed
`tools.toml` contains the command line templates of all the tested tools
`local.example.toml` contains local references to the datasets and working directories used in benchmarks.
This last file should be edited (and renamed to `local.toml`) adjusting the local paths to the datasets

## Data availability

The sources download the used datasets are available under the datasets-download/ directory
