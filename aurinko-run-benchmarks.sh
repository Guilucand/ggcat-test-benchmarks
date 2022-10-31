#!/usr/bin/bash

mkdir article-test-{d,e,f}/
cargo run --release -- bench article-test-d-k27 article-test-d/
cargo run --release -- bench article-test-e-k27 article-test-e/

cargo run --release -- bench article-test-d-k63 article-test-d/
cargo run --release -- bench article-test-e-k63 article-test-e/


cargo run --release -- bench article-test-f article-test-f/