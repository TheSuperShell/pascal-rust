
default rel

section .rdata
    fmt_i64 db "%lld", 0
    fmt_c db "%c", 0
    fmt_nl db 10, 0

section .text
global std.io.writeln
global std.io.write
extern printf

std.io.write:
    sub rsp, 32

    mov rdx, rcx
    mov rcx, [fmt_i64]
    call printf

    add rsp, 32
    ret


std.io.writeln:
    sub rsp, 32

    mov rdx, rcx
    lea rcx, [fmt_i64]
    call printf

    lea rcx, [fmt_nl]
    call printf

    add rsp, 32
    ret