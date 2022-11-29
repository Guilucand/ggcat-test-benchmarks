#!/usr/bin/bash

mkdir article-test-g/
cargo run --release -- bench article-test-g article-test-g/ --threads $1