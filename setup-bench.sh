
# Append GCC 13 compatibility fixes to cuttlefish's KMC patch file if not already present.
# KMC 3.2.1 is missing <stdexcept> in critical_error_handler.h and <cstdint> in kff_writer.h.
patch_kmc_for_gcc13() {
    local patchfile="$1"
    [ -f "$patchfile" ] || return 0  # no patch file, skip
    grep -q "stdexcept" "$patchfile" && return  # already patched
    cat >> "$patchfile" << 'PATCH'
diff --git a/kmc_core/critical_error_handler.h b/kmc_core/critical_error_handler.h
--- a/kmc_core/critical_error_handler.h
+++ b/kmc_core/critical_error_handler.h
@@ -1,6 +1,7 @@
 #pragma once

 #include <set>
 #include <mutex>
+#include <stdexcept>
 #include "thread_cancellation_exception.h" //TODO: moze ten wyjatek zdefiniowac tutaj?
 #include <condition_variable>
diff --git a/kmc_core/kff_writer.h b/kmc_core/kff_writer.h
--- a/kmc_core/kff_writer.h
+++ b/kmc_core/kff_writer.h
@@ -1,4 +1,5 @@
 #pragma once

+#include <cstdint>
 #include <string>
 #include <vector>
PATCH
}

patch_cuttlefish3_streams() {
    local source_dir="$1"
    local patchfile="$2"

    if git -C "$source_dir" apply --reverse --check "$patchfile" 2>/dev/null; then
        return 0  # already patched
    fi

    git -C "$source_dir" apply "$patchfile"
}

cargo build --release

# System dependencies (cuttlefish3 branch requires liblz4-dev and nasm)
apt-get install -y libbz2-dev autoconf liblz4-dev nasm 2>/dev/null || true

mkdir -p building/
mkdir -p tools/

pushd building/

    [ -d ggcat1 ]      || git clone https://github.com/algbio/ggcat --recursive ggcat1
    [ -d ggcat2 ]      || git clone https://github.com/algbio/ggcat --recursive ggcat2
    [ -d ggcat2.0 ]    || git clone --branch v2.0.0 https://github.com/algbio/ggcat --recursive ggcat2.0
    [ -d bcalm ]       || git clone https://github.com/GATB/bcalm --recursive
    [ -d bifrost ]     || git clone https://github.com/pmelsted/bifrost
    [ -d bifrost-k63 ] || git clone https://github.com/pmelsted/bifrost bifrost-k63
    [ -d cuttlefish2 ] || { git clone https://github.com/COMBINE-lab/cuttlefish cuttlefish2 && patch_kmc_for_gcc13 cuttlefish2/patches/kmc_patch.diff; }
    [ -d cuttlefish3 ] || { git clone --branch cuttlefish3 https://github.com/COMBINE-lab/cuttlefish cuttlefish3 && patch_kmc_for_gcc13 cuttlefish3/patches/kmc_patch.diff; }
    patch_cuttlefish3_streams cuttlefish3 ../../patches/cuttlefish3_stream_close.patch

    if [ ! -f ../tools/ggcat1 ]; then
        pushd ggcat1/
            git fetch --tags
            git checkout v1.1.1
            # v1.1.1 depends on an older parallel-processor crate where Stat::from_reader
            # was renamed to from_read; build without process-stats to avoid the conflict
            cargo build --release
            cp ./target/release/ggcat ../../tools/ggcat1 -f
        popd
    fi

    if [ ! -f ../tools/ggcat2 ]; then
        pushd ggcat2/
            git checkout main
            git pull
            cargo build --release --features "process-stats"
            cp ./target/release/ggcat ../../tools/ggcat2 -f
        popd
    fi

    if [ ! -f ../tools/ggcat2.0 ]; then
        pushd ggcat2.0/
            git fetch --tags
            # GGCAT version used in https://doi.org/10.1101/2025.02.02.636161
            # (v2.0.0 resolves to 08ccdb672916150e52bada9637bd5cb98c17f247).
            git checkout v2.0.0
            cargo build --release --features "process-stats"
            cp ./target/release/ggcat ../../tools/ggcat2.0 -f
        popd
    fi

    if [ ! -f ../tools/bcalm ]; then
        pushd bcalm/
            git pull
            mkdir -p build && cd build
            cmake .. -DKSIZE_LIST="32 64 96 128 160 192 224 256"
            make -j
            cp bcalm ../../../tools/bcalm -f
        popd
    fi

    if [ ! -f ../tools/Bifrost ]; then
        pushd bifrost/
            git pull
            mkdir -p build && cd build
            cmake ..
            make -j
            cp src/Bifrost ../../../tools/Bifrost -f
        popd
    fi

    if [ ! -f ../tools/Bifrost-k63 ]; then
        pushd bifrost-k63/
            git pull
            mkdir -p build && cd build
            cmake .. -DMAX_KMER_SIZE=64
            make -j
            cp src/Bifrost ../../../tools/Bifrost-k63 -f
        popd
    fi

    if [ ! -f ../tools/cuttlefish ]; then
        pushd cuttlefish2/
            git pull
            patch_kmc_for_gcc13 patches/kmc_patch.diff
            mkdir -p build && cd build
            cmake .. -DINSTANCE_COUNT=256
            make -j
            cp src/cuttlefish ../../../tools/cuttlefish -f
        popd
    fi

    if [ ! -f ../tools/cuttlefish3-bin ]; then
        pushd cuttlefish3/
            git pull
            patch_kmc_for_gcc13 patches/kmc_patch.diff
            mkdir -p build && cd build
            cmake ..
            make -j
            cp src/cuttlefish ../../../tools/cuttlefish3-bin -f
        popd
    fi
    # Install wrapper script (always, so updates are picked up)
    cp ../scripts/cuttlefish3-wrapper.sh ../tools/cuttlefish3
    chmod +x ../tools/cuttlefish3

popd
