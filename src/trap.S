# Ensamblador de bajo nivel para capturar trampas
.section .text
.global trap_entry
.align 4
trap_entry:
    # 1. Reservar espacio en la pila del kernel para el TrapFrame (34 * 8 = 272 bytes)
    addi sp, sp, -272

    # 2. Guardar todos los registros de propósito general (excepto x0 y sp)
    sd x1, 8(sp)
    # x2 (sp) lo guardaremos de forma separada abajo con el valor previo a la resta
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

    # Calcular y guardar el sp original (antes del trap)
    addi t0, sp, 272
    sd t0, 16(sp)

    # 3. Leer y guardar CSRs de estatus del trap
    csrr t0, sstatus
    sd t0, 256(sp)
    csrr t1, sepc
    sd t1, 264(sp)

    # 4. Establecer el primer argumento (a0) como el puntero al TrapFrame
    mv a0, sp
    call rust_trap_handler

.global trap_return
trap_return:
    # 5. Restaurar CSRs
    ld t0, 256(sp)
    csrw sstatus, t0
    ld t1, 264(sp)
    csrw sepc, t1

    # 6. Restaurar todos los registros de propósito general (excepto sp)
    ld x1, 8(sp)
    # x2 (sp) lo restauramos al final con addi
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

    # 7. Restaurar el puntero de pila y volver del modo Supervisor
    addi sp, sp, 272
    sret
