default rel

section .rdata
div0_msg db "Runtime error: division by zero"
div0_len equ $ - div0_msg

section .text
global std.error.div0_error

extern GetStdHandle
extern WriteFile
extern ExitProcess

STD_ERROR_HANDLE equ -12

std.error.div0_error:
    sub rsp, 72
    
    mov ecx, STD_ERROR_HANDLE
    call GetStdHandle

    mov rcx, rax
    lea rdx, [div0_msg]
    mov r8d, div0_len
    lea r9, [rsp + 40]

    mov qword [rsp + 32], 0
    call WriteFile

    add rsp, 72
    mov ecx, 1
    call ExitProcess
