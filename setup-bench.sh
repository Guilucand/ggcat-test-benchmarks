
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

cargo build --release

# System dependencies (cuttlefish3 branch requires liblz4-dev and nasm)
apt-get install -y libbz2-dev autoconf liblz4-dev nasm 2>/dev/null || true

mkdir -p building/
mkdir -p tools/

pushd building/

    [ -d ggcat ]       || git clone https://github.com/algbio/ggcat --recursive
    [ -d bcalm ]       || git clone https://github.com/GATB/bcalm --recursive
    [ -d bifrost ]     || git clone https://github.com/pmelsted/bifrost
    [ -d bifrost-k63 ] || git clone https://github.com/pmelsted/bifrost bifrost-k63
    [ -d cuttlefish2 ] || { git clone https://github.com/COMBINE-lab/cuttlefish cuttlefish2 && patch_kmc_for_gcc13 cuttlefish2/patches/kmc_patch.diff; }
    [ -d cuttlefish3 ] || { git clone --branch cuttlefish3 https://github.com/COMBINE-lab/cuttlefish cuttlefish3 && patch_kmc_for_gcc13 cuttlefish3/patches/kmc_patch.diff; }

    if [ ! -f ../tools/ggcat ]; then
        pushd ggcat/
            git pull
            git checkout dev
            cargo build --release --features "process-stats"
            cp ./target/release/ggcat ../../tools/ggcat -f
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

    if [ ! -f ../tools/cuttlefish3 ]; then
        pushd cuttlefish3/
            git pull
            patch_kmc_for_gcc13 patches/kmc_patch.diff
            mkdir -p build && cd build
            cmake ..
            make -j
            cp src/cuttlefish ../../../tools/cuttlefish3 -f
        popd
    fi

popd
