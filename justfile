build:
    cargo build

run:
    cargo run

[group('testing')]
test:
    cargo test

[group('testing')]
cov:
    cargo llvm-cov --open

[group('testing')]
bench:
    cargo bench