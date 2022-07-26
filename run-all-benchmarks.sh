
mkdir tests-final/

cargo run --release -- bench salmonella-small tests-final/salmonella-small
cargo run --release -- bench colored-salmonella-small tests-final/colored-salmonella-small

cargo run --release -- bench salmonella-big tests-final/salmonella-big
cargo run --release -- bench colored-salmonella-big tests-final/colored-salmonella-big

cargo run --release -- bench gut-complete tests-final/gut-complete

cargo run --release -- bench human tests-final/human

cargo run --release -- bench salmonella-huge tests-final/salmonella-huge
cargo run --release -- bench colored-salmonella-huge tests-final/colored-salmonella-huge

