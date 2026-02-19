build:
    cargo build

run source *ARGS:
    cargo run interp {{source}} {{ARGS}}

run-compile source target *ARGS:
    cargo run compile {{source}} target/{{target}}.asm {{ARGS}}
    nasm -f win64 -o target/compiled.obj target/{{target}}.asm
    gcc -o target/{{target}}.exe target/compiled.obj
    rm target/compiled.obj
    ./target/{{target}}.exe


[group('asm')]
compile-asm asm:
    nasm -f win64 -o compiled.obj {{asm}}
    gcc -o result compiled.obj; rm compiled.obj
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

[group('testing')]
bench:
    cargo bench