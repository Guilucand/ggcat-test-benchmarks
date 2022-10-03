
mkdir tests-final/

cargo run --release -- bench human-colored tests-final/human-colored --threads "$SLURM_CPUS_ON_NODE"
cargo run --release -- bench human-colored-k63 tests-final/human-colored-k63 --threads "$SLURM_CPUS_ON_NODE"

cargo run --release -- bench colored-salmonella-big tests-final/colored-salmonella-big --threads "$SLURM_CPUS_ON_NODE"
cargo run --release -- bench colored-salmonella-huge tests-final/colored-salmonella-huge --threads "$SLURM_CPUS_ON_NODE"

