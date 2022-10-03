
mkdir tests-final/

cargo run --release -- bench human-colored tests-final/human-colored
cargo run --release -- bench human-colored-k63 tests-final/human-colored-k63

cargo run --release -- bench colored-salmonella-big tests-final/colored-salmonella-big
cargo run --release -- bench colored-salmonella-huge tests-final/colored-salmonella-huge

