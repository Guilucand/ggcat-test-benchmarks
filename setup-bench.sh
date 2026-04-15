
cargo build --release

mkdir -p building/
mkdir -p tools/

pushd building/

    [ -d ggcat ]       || git clone https://github.com/algbio/ggcat --recursive
    [ -d bcalm ]       || git clone https://github.com/GATB/bcalm --recursive
    [ -d bifrost ]     || git clone https://github.com/pmelsted/bifrost
    [ -d bifrost-k63 ] || git clone https://github.com/pmelsted/bifrost bifrost-k63
    [ -d cuttlefish2 ] || git clone https://github.com/COMBINE-lab/cuttlefish cuttlefish2
    if [ ! -d cuttlefish3 ]; then
        git clone https://github.com/COMBINE-lab/cuttlefish3 cuttlefish3 \
            || git clone https://github.com/COMBINE-lab/cuttlefish cuttlefish3
    fi

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
            mkdir -p build && cd build
            cmake .. -DINSTANCE_COUNT=256
            make -j
            cp src/cuttlefish ../../../tools/cuttlefish -f
        popd
    fi

    if [ ! -f ../tools/cuttlefish3 ]; then
        pushd cuttlefish3/
            git pull
            mkdir -p build && cd build
            cmake ..
            make -j
            cp src/cuttlefish ../../../tools/cuttlefish3 -f
        popd
    fi

popd
