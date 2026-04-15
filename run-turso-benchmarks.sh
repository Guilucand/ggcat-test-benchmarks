#!/usr/bin/bash
set -e

log() { echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*"; }

log "=== Starting turso benchmarks ==="

# Generate file lists for datasets that span many individual files
log "Generating file lists..."
mkdir -p config/lists

# ecoli: 3681 reference files named reference-0.fa .. reference-3680.fa
if [ -f config/lists/ecoli.in ]; then
    log "  ecoli.in already exists ($(wc -l < config/lists/ecoli.in) entries), skipping"
else
    log "  Building ecoli file list (3681 files)..."
    ECOLI_DIR="/home/ad/turso/wrk-vakka/users/sebschmi/matchtigs/genomes/gecoli3681-c3681-i0-e0"
    for i in $(seq 0 3680); do
        echo "${ECOLI_DIR}/reference-${i}.fa"
    done > config/lists/ecoli.in
    log "  ecoli.in written ($(wc -l < config/lists/ecoli.in) entries)"
fi

# salmonella-550k: fasta files spread across 550 subdirectories
if [ -f config/lists/salmonella-550k.in ]; then
    log "  salmonella-550k.in already exists ($(wc -l < config/lists/salmonella-550k.in) entries), skipping"
else
    log "  Building salmonella-550k file list (scanning 550 dirs)..."
    SALMONELLA_BASE="/home/ad/turso/wrk-vakka/users/sebschmi/matchtigs/downloads/enterobase_salmonella"
    for i in $(seq 0 549); do
        find "${SALMONELLA_BASE}/extracted_cleaned_${i}/" -type f \( -name "*.fa" -o -name "*.fasta" \)
    done > config/lists/salmonella-550k.in
    log "  salmonella-550k.in written ($(wc -l < config/lists/salmonella-550k.in) entries)"
fi

log "Installing tool wrappers..."
mkdir -p tools
cp scripts/cuttlefish3-wrapper.sh tools/cuttlefish3
chmod +x tools/cuttlefish3
log "Wrappers installed."

log "Building benchmark runner..."
cargo build --release
log "Build complete."

LOCAL_CFG="config/turso-local.toml"

mkdir -p bench-results/{ecoli,human100,human2505,salmonella-550k,ena-bacteria,all-the-bacteria}

log "--- Running ecoli benchmark ---"
cargo run --release -- bench ecoli-bench          bench-results/ecoli/           -e "$LOCAL_CFG"
log "--- ecoli benchmark done ---"

log "--- Running human100 benchmark ---"
cargo run --release -- bench human100-bench       bench-results/human100/        -e "$LOCAL_CFG"
log "--- human100 benchmark done ---"

log "--- Running human2505 benchmark ---"
cargo run --release -- bench human2505-bench      bench-results/human2505/       -e "$LOCAL_CFG"
log "--- human2505 benchmark done ---"

log "--- Running salmonella-550k benchmark ---"
cargo run --release -- bench salmonella-550k-bench bench-results/salmonella-550k/ -e "$LOCAL_CFG"
log "--- salmonella-550k benchmark done ---"

log "--- Running ena-bacteria benchmark ---"
cargo run --release -- bench ena-bacteria-bench   bench-results/ena-bacteria/    -e "$LOCAL_CFG"
log "--- ena-bacteria benchmark done ---"

log "--- Running all-the-bacteria benchmark ---"
cargo run --release -- bench all-the-bacteria-bench bench-results/all-the-bacteria/ -e "$LOCAL_CFG"
log "--- all-the-bacteria benchmark done ---"

log "=== All benchmarks complete ==="
