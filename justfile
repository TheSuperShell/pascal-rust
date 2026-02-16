build:
    cargo build

run source target:
    cargo run {{ source }} {{ target }}

run-compile source target:
    cargo run {{source}} target/{{target}}.asm
    nasm -f win64 -o target/compiled.obj target/{{target}}.asm
    gcc -o target/{{target}}.exe target/compiled.obj
    rm target/compiled.obj
    ./target/{{target}}.exe

[group('testing')]
compile-asm asm:
    nasm -f win64 -o compiled.obj {{asm}}
    gcc -o result compiled.obj
    rm compiled.obj

[group('testing')]
test filter="" $RUST_BACKTRACE="1":
    cargo test {{ filter }}

[group('testing')]
cov:
    cargo llvm-cov --open

[group('testing')]
bench:
    cargo bench