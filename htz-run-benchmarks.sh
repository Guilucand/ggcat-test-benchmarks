#!/usr/bin/bash

mkdir article-test-{a,b,c}/
cargo run --release -- bench article-test-a article-test-a/
cargo run --release -- bench article-test-b article-test-b/
cargo run --release -- bench article-test-c article-test-c/