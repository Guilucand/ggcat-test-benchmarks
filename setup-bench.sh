
cargo build --release

mkdir building/
mkdir tools/

pushd building/

    git clone https://github.com/Guilucand/ggcat --recursive
    git clone https://github.com/GATB/bcalm --recursive
    git clone https://github.com/pmelsted/bifrost
    git clone https://github.com/pmelsted/bifrost-k63
    git clone https://github.com/COMBINE-lab/cuttlefish cuttlefish2

    pushd ggcat/
        git pull
	git checkout main
        cargo build --release --features "process-stats"
        cp ./target/release/ggcat ../../tools/ggcat -f
        # cargo build --release --features "build-links,process-stats"
        # cp ./target/release/ggcat ../../tools/ggcat-links
    popd

    pushd bcalm/
        git pull
        mkdir build
        cd build
        cmake .. -DKSIZE_LIST="32 64 96 128 160 192 224 256"
        make -j
    popd

    pushd bifrost/
        git pull
        mkdir build
        cd build
        cmake ..
        make -j
    popd

    pushd bifrost-k63/
        git pull
        mkdir build
        cd build
        cmake .. -DMAX_KMER_SIZE=64
        make -j
    popd


    pushd cuttlefish2/
        git pull
        mkdir build
        cd build
        cmake .. -DINSTANCE_COUNT=256
        make -j
    popd
popd

cp building/bifrost/build/src/Bifrost tools/ -f
cp building/bcalm/build/bcalm tools/ -f
cp building/cuttlefish2/build/src/cuttlefish tools/ -f
