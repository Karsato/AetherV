OUTPUT_ARCH(riscv)
ENTRY(_start)

BASE_ADDRESS = 0x80200000;

SECTIONS
{
    . = BASE_ADDRESS;

    . = ALIGN(4096);
    .text : {
        stext = .;
        *(.text.init)
        *(.text .text.*)
        . = ALIGN(4096);
        etext = .;
    }

    . = ALIGN(4096);
    .rodata : {
        srodata = .;
        *(.rodata .rodata.*)
        . = ALIGN(4096);
        erodata = .;
    }

    . = ALIGN(4096);
    .data : {
        sdata = .;
        *(.data .data.*)
        . = ALIGN(4096);
        edata = .;
    }

    . = ALIGN(4096);
    .bss : {
        sbss = .;
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(4096);
        ebss = .;
    }

    . = ALIGN(4096);
    ekernel = .;

    /DISCARD/ : {
        *(.eh_frame)
    }
}
