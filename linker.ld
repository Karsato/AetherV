OUTPUT_ARCH(riscv)
ENTRY(_start)

BASE_ADDRESS = 0x80200000;

SECTIONS
{
    . = BASE_ADDRESS;

    .text : {
        *(.text.init)
        *(.text .text.*)
    }

    .rodata : {
        *(.rodata .rodata.*)
    }

    .data : {
        *(.data .data.*)
    }

    .bss : {
        sbss = .;
        *(.bss .bss.*)
        *(COMMON)
        ebss = .;
    }

    /DISCARD/ : {
        *(.eh_frame)
    }
}
