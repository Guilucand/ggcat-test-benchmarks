
cargo build --release

mkdir building/
mkdir tools/

pushd building/

    pushd biloki/
        git pull
        cargo build --release --features "process-stats"
        cp ./target/release/biloki ../../tools/biloki -f
        # cargo build --release --features "build-links,process-stats"
        # cp ./target/release/biloki ../../tools/biloki-links
    popd
popd
