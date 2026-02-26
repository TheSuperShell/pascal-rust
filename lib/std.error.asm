default rel

section .rdata
div0_msg db "Runtime error: division by zero", 0

section .text
global std.error.div0_error
extern puts
extern ExitProcess

std.error.div0_error:
    sub rsp, 32
    lea rcx, [div0_msg]
    call puts
    add rsp, 32
    mov ecx, 1
    call ExitProcess
