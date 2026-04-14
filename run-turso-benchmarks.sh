#!/usr/bin/bash
set -e

# Generate file lists for datasets that span many individual files
mkdir -p config/lists

# ecoli: 3681 reference files named reference-0.fa .. reference-3680.fa
ECOLI_DIR="/home/ad/turso/wrk-vakka/users/sebschmi/matchtigs/genomes/gecoli3681-c3681-i0-e0"
for i in $(seq 0 3680); do
    echo "${ECOLI_DIR}/reference-${i}.fa"
done > config/lists/ecoli.in

# salmonella-550k: fasta files spread across 550 subdirectories
SALMONELLA_BASE="/home/ad/turso/wrk-vakka/users/sebschmi/matchtigs/downloads/enterobase_salmonella"
for i in $(seq 0 549); do
    find "${SALMONELLA_BASE}/extracted_cleaned_${i}/" -type f \( -name "*.fa" -o -name "*.fasta" \)
done > config/lists/salmonella-550k.in

cargo build --release

LOCAL_CFG="config/turso-local.toml"

mkdir -p bench-results/{ecoli,human100,human2505,salmonella-550k,ena-bacteria,all-the-bacteria}

cargo run --release -- bench ecoli-bench          bench-results/ecoli/           -e "$LOCAL_CFG"
cargo run --release -- bench human100-bench       bench-results/human100/        -e "$LOCAL_CFG"
cargo run --release -- bench human2505-bench      bench-results/human2505/       -e "$LOCAL_CFG"
cargo run --release -- bench salmonella-550k-bench bench-results/salmonella-550k/ -e "$LOCAL_CFG"
cargo run --release -- bench ena-bacteria-bench   bench-results/ena-bacteria/    -e "$LOCAL_CFG"
cargo run --release -- bench all-the-bacteria-bench bench-results/all-the-bacteria/ -e "$LOCAL_CFG"
