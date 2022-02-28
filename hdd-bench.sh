cargo run --release bench gut --include hdd --exclude bcalm hdd-gut --threads 12
cargo run --release bench human --include hdd --exclude bcalm hdd-human --threads 12
cargo run --release bench salmonella-small --include hdd --exclude bcalm hdd-salmonella-small --threads 12
cargo run --release bench salmonella-big --include hdd --exclude bcalm hdd-salmonella-big --threads 12
