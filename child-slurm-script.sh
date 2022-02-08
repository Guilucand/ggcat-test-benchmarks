#!/bin/bash

export LD_LIBRARY_PATH=/wrk/users/acracco/biloki-test/assemblers-benchmark/building/gcc/lib64

cargo run --release -- bench gut gut-bench-results
cargo run --release -- bench human human-bench-results
cargo run --release -- bench human-reassemble human-reassemble-bench-results
cargo run --release -- bench salmonella-small salmonella-small-bench-results
cargo run --release -- bench salmonella-big salmonella-big-bench-results
