build:
    cargo build

run:
    cargo run

[group('testing')]
test filter="" $RUST_BACKTRACE="1":
    cargo test {{ filter }}

[group('testing')]
cov:
    cargo llvm-cov --open

[group('testing')]
bench:
    cargo bench