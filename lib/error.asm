default rel

section .rdata
div0_msg db "Runtime error: division by zero", 10, 0

section .text
global div0_error
extern puts
extern ExitProcess

div0_error:
    sub rsp, 40
    lea rcx, [div0_msg]
    call puts
    mov ecx, 1
    call ExitProcess
