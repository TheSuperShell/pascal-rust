build:
    cargo build

run source *ARGS:
    cargo run interp {{source}} {{ARGS}}

run-compile source target *ARGS:
    cargo run compile {{source}} target/{{target}}.asm {{ARGS}}
    nasm -f win64 -o target/compiled.obj target/{{target}}.asm
    nasm -f win64 -o target/error.obj lib/std.error.asm
    gcc -o target/{{target}}.exe target/error.obj target/compiled.obj
    rm target/compiled.obj target/error.obj
    ./target/{{target}}.exe || echo "Program existed with code $?"


[group('asm')]
compile-asm asm:
    nasm -f win64 -o compiled.obj {{asm}}
    nasm -f win64 -o error.obj lib/std.error.asm
    gcc -o result compiled.obj error.obj; rm compiled.obj error.obj
    ./result.exe
    rm result.exe

[group('asm')]
c-asm source target="target.asm":
    gcc -O0 -masm=intel -S {{source}} -o {{target}}
    code {{ target }}

[group('testing')]
test filter="" $RUST_BACKTRACE="1":
    cargo test {{ filter }}

[group('testing')]
cov:
    cargo llvm-cov --open

[group('bench')]
bench:
    cargo bench

[group('bench')]
bench-compile num="100" bench_file="benches/bench.pas":
    just run-compile {{bench_file}} bench >/dev/null
    benches/bench.sh {{num}} target/bench.exe

[group('bench')]
bench-interp num="100" bench_file="benches/bench.pas":
    cargo build --release
    benches/bench.sh {{num}} "target/release/pascal-rust.exe interp {{bench_file}}"