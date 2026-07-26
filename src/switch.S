# Ensamblador de bajo nivel para cambio de contexto de tareas y lanzador de entrada
.section .text
.global switch_to
.align 4
switch_to:
    # a0: Puntero al TaskContext de la tarea antigua (guardar)
    # a1: Puntero al TaskContext de la tarea nueva (restaurar)

    # 1. Guardar registros preservados de la tarea antigua
    sd ra, 0(a0)
    sd sp, 8(a0)
    sd s0, 16(a0)
    sd s1, 24(a0)
    sd s2, 32(a0)
    sd s3, 40(a0)
    sd s4, 48(a0)
    sd s5, 56(a0)
    sd s6, 64(a0)
    sd s7, 72(a0)
    sd s8, 80(a0)
    sd s9, 88(a0)
    sd s10, 96(a0)
    sd s11, 104(a0)

    # 2. Restaurar registros de la nueva tarea
    ld ra, 0(a1)
    ld sp, 8(a1)
    ld s0, 16(a1)
    ld s1, 24(a1)
    ld s2, 32(a1)
    ld s3, 40(a1)
    ld s4, 48(a1)
    ld s5, 56(a1)
    ld s6, 64(a1)
    ld s7, 72(a1)
    ld s8, 80(a1)
    ld s9, 88(a1)
    ld s10, 96(a1)
    ld s11, 104(a1)

    ret

.global task_entry_helper
task_entry_helper:
    # s0 contiene el puntero de la función de entrada.
    # Lo llamamos utilizando el registro de salto jalr.
    jalr s0

    # Si la función de la tarea retorna, llamamos al limpiador en Rust.
    call rust_exit_current_task
