#!/usr/bin/bash

mkdir article-test-{d,e,f}/
cargo run --release -- bench article-test-d article-test-d/
cargo run --release -- bench article-test-e article-test-e/
cargo run --release -- bench article-test-f article-test-f/