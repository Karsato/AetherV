# Ensamblador de bajo nivel para capturar trampas (S-Mode y U-Mode)
.section .text
.global trap_entry
.align 4
trap_entry:
    # Intercambiar sp con sscratch.
    # Si venimos de U-mode, sp ahora apunta al tope de la pila del kernel de la tarea,
    # y sscratch apunta a la pila de usuario.
    # Si venimos de S-mode, sscratch era 0, por lo que sp ahora es 0, y sscratch tiene la pila del kernel.
    csrrw sp, sscratch, sp

    # Comprobar si venimos de S-mode (sp == 0)
    bnez sp, trap_from_user

trap_from_kernel:
    # Restaurar sp y poner sscratch de nuevo a 0
    csrrw sp, sscratch, sp
    
    # Reservar TrapFrame en la pila del kernel (34 * 8 = 272 bytes)
    addi sp, sp, -272
    
    # Guardar registros x1, x3-x31
    sd x1, 8(sp)
    # Guardar sp original (sp + 272)
    addi t0, sp, 272
    sd t0, 16(sp)
    
    j save_rest_and_call

trap_from_user:
    # sp ya apunta al tope de la pila del kernel de la tarea.
    # Reservar TrapFrame en la pila del kernel.
    addi sp, sp, -272

    # Guardar x1
    sd x1, 8(sp)

    # Leer pila de usuario original desde sscratch y guardarla en sp del TrapFrame
    csrr t0, sscratch
    sd t0, 16(sp)

    # Poner sscratch a 0 mientras estamos en el kernel
    li t0, 0
    csrw sscratch, t0

save_rest_and_call:
    sd x3, 24(sp)
    sd x4, 32(sp)
    sd x5, 40(sp)
    sd x6, 48(sp)
    sd x7, 56(sp)
    sd x8, 64(sp)
    sd x9, 72(sp)
    sd x10, 80(sp)
    sd x11, 88(sp)
    sd x12, 96(sp)
    sd x13, 104(sp)
    sd x14, 112(sp)
    sd x15, 120(sp)
    sd x16, 128(sp)
    sd x17, 136(sp)
    sd x18, 144(sp)
    sd x19, 152(sp)
    sd x20, 160(sp)
    sd x21, 168(sp)
    sd x22, 176(sp)
    sd x23, 184(sp)
    sd x24, 192(sp)
    sd x25, 200(sp)
    sd x26, 208(sp)
    sd x27, 216(sp)
    sd x28, 224(sp)
    sd x29, 232(sp)
    sd x30, 240(sp)
    sd x31, 248(sp)

    # Guardar CSRs sstatus y sepc
    csrr t0, sstatus
    sd t0, 256(sp)
    csrr t1, sepc
    sd t1, 264(sp)

    # Establecer el argumento a0 como puntero al TrapFrame
    mv a0, sp
    call rust_trap_handler

.global trap_return
trap_return:
    # Comprobar si volvemos a U-mode leyendo el bit SPP de sstatus guardado
    ld t0, 256(sp) # sstatus guardado
    csrw sstatus, t0

    ld t1, 264(sp) # sepc guardado
    csrw sepc, t1

    # Cargar bit SPP (bit 8 de sstatus)
    andi t0, t0, 0x100
    bnez t0, return_to_kernel

return_to_user:
    # Si volvemos a U-mode, debemos restaurar la pila de kernel en sscratch (sp + 272)
    addi t0, sp, 272
    csrw sscratch, t0

    # Restaurar todos los registros de propósito general excepto sp
    ld x1, 8(sp)
    ld x3, 24(sp)
    ld x4, 32(sp)
    ld x5, 40(sp)
    ld x6, 48(sp)
    ld x7, 56(sp)
    ld x8, 64(sp)
    ld x9, 72(sp)
    ld x10, 80(sp)
    ld x11, 88(sp)
    ld x12, 96(sp)
    ld x13, 104(sp)
    ld x14, 112(sp)
    ld x15, 120(sp)
    ld x16, 128(sp)
    ld x17, 136(sp)
    ld x18, 144(sp)
    ld x19, 152(sp)
    ld x20, 160(sp)
    ld x21, 168(sp)
    ld x22, 176(sp)
    ld x23, 184(sp)
    ld x24, 192(sp)
    ld x25, 200(sp)
    ld x26, 208(sp)
    ld x27, 216(sp)
    ld x28, 224(sp)
    ld x29, 232(sp)
    ld x30, 240(sp)
    ld x31, 248(sp)

    # Cargar pila de usuario
    ld sp, 16(sp)
    sret

return_to_kernel:
    # Si volvemos a S-mode, sscratch sigue siendo 0, no tocamos sscratch.
    ld x1, 8(sp)
    ld x3, 24(sp)
    ld x4, 32(sp)
    ld x5, 40(sp)
    ld x6, 48(sp)
    ld x7, 56(sp)
    ld x8, 64(sp)
    ld x9, 72(sp)
    ld x10, 80(sp)
    ld x11, 88(sp)
    ld x12, 96(sp)
    ld x13, 104(sp)
    ld x14, 112(sp)
    ld x15, 120(sp)
    ld x16, 128(sp)
    ld x17, 136(sp)
    ld x18, 144(sp)
    ld x19, 152(sp)
    ld x20, 160(sp)
    ld x21, 168(sp)
    ld x22, 176(sp)
    ld x23, 184(sp)
    ld x24, 192(sp)
    ld x25, 200(sp)
    ld x26, 208(sp)
    ld x27, 216(sp)
    ld x28, 224(sp)
    ld x29, 232(sp)
    ld x30, 240(sp)
    ld x31, 248(sp)

    addi sp, sp, 272
    sret
