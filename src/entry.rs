core::arch::global_asm!(
    r#"
    .section .text.init
    .global _start
_start:
    # Setup stack pointer
    la sp, boot_stack_top
    
    # Pass hartid (a0) and fdt (a1) to main
    call rust_main

loop:
    wfi
    j loop

    .section .bss
    .align 12
boot_stack:
    .space 4096 * 2
boot_stack_top:
"#
);
