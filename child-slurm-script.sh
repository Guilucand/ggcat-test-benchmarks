#!/bin/bash

export LD_LIBRARY_PATH=/wrk/users/acracco/biloki-test/assemblers-benchmark/building/gcc/lib64

cargo run --release -- bench gut gut-bench-results --include "cuttlefish2-reads,biloki,ram,hdd" --threads "$SLURM_CPUS_ON_NODE"
cargo run --release -- bench human human-bench-results --include "cuttlefish2-reads,biloki,ram,hdd" --threads "$SLURM_CPUS_ON_NODE"
cargo run --release -- bench human-reassemble human-reassemble-bench-results --include "cuttlefish2-ref,biloki,ram,hdd" --threads "$SLURM_CPUS_ON_NODE"
cargo run --release -- bench salmonella-small salmonella-small-bench-results --include "cuttlefish2-ref,biloki,ram,hdd" --threads "$SLURM_CPUS_ON_NODE"
cargo run --release -- bench salmonella-big salmonella-big-bench-results --include "cuttlefish2-ref,biloki,ram,hdd" --threads "$SLURM_CPUS_ON_NODE"
cargo run --release -- bench salmonella-small salmonella-small-bench-results --include "salmonella-1k,salmonella-10k,bcalm,ram" --threads "$SLURM_CPUS_ON_NODE"
