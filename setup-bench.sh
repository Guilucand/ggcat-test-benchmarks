
cargo build --release

mkdir building/
mkdir tools/

pushd building/

    git clone https://github.com/Guilucand/biloki --recursive
    git clone https://github.com/GATB/bcalm --recursive
    git clone https://github.com/pmelsted/bifrost
    git clone https://github.com/COMBINE-lab/cuttlefish cuttlefish2

    pushd biloki/
        cargo build --release --features build-links
    popd

    pushd bcalm/
        mkdir build
        cd build
        cmake ..
        make -j
    popd

    pushd bifrost/
        mkdir build
        cd build
        cmake ..
        make -j
    popd
    cp bifrost/ tools


    pushd cuttlefish2/
        mkdir build
        cd build
        cmake ..
        make -j
    popd
popd

cp building/biloki/target/release/biloki tools/
cp building/bifrost/build/src/Bifrost tools/
cp building/bcalm/build/bcalm tools/
cp building/cuttlefish2/build/src/cuttlefish tools/
