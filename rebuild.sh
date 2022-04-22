
cargo build --release

mkdir building/
mkdir tools/

pushd building/

    pushd ggcat/
        git pull
        cargo build --release --features "process-stats"
        cp ./target/release/ggcat ../../tools/ggcat -f
        # cargo build --release --features "build-links,process-stats"
        # cp ./target/release/ggcat ../../tools/ggcat-links
    popd
popd
