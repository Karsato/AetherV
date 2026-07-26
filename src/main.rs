#![no_std]
#![no_main]

mod entry;
mod sbi;
mod trap;
mod paging;

use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn rust_main(_hart_id: usize, _fdt_ptr: usize) -> ! {
    // Zero out BSS section
    extern "C" {
        static mut sbss: u8;
        static mut ebss: u8;
    }
    unsafe {
        let sbss_ptr = core::ptr::addr_of_mut!(sbss);
        let ebss_ptr = core::ptr::addr_of_mut!(ebss);
        if sbss_ptr < ebss_ptr {
            let count = ebss_ptr as usize - sbss_ptr as usize;
            core::ptr::write_bytes(sbss_ptr, 0, count);
        }
    }

    sbi::print_str("\n========================================\n");
    sbi::print_str("  MicroRust Kernel Initialized (RISC-V) \n");
    sbi::print_str("========================================\n");
    
    // Inicializar el sistema de trampas (Trap Handler)
    trap::init();
    sbi::print_str("[Kernel] Sistema de trampas inicializado.\n");

    // Inicializar memoria virtual (Paginación Sv39)
    paging::init();

    // Activar interrupción del temporizador
    trap::enable_timer_interrupt();
    sbi::print_str("[Kernel] Interrupciones de reloj activadas.\n");

    // Prueba 1: realizar una llamada al sistema ficticia (ecall) para verificar
    sbi::print_str("[Kernel] Probando ecall ficticio en Supervisor Mode...\n");
    unsafe {
        core::arch::asm!("ecall");
    }
    sbi::print_str("[Kernel] Retorno de ecall exitoso!\n");

    // Prueba 2: realizar un ebreak (breakpoint) en S-mode para verificar
    sbi::print_str("[Kernel] Probando ebreak (breakpoint) en S-mode...\n");
    unsafe {
        core::arch::asm!("ebreak");
    }
    sbi::print_str("[Kernel] Retorno de ebreak exitoso!\n");

    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    sbi::print_str("\n[KERNEL PANIC]: ");
    if let Some(location) = info.location() {
        sbi::print_str("Execution halted at ");
        sbi::print_str(location.file());
        sbi::print_str(":");
        sbi::print_hex(location.line() as usize);
    } else {
        sbi::print_str("Execution halted.");
    }
    sbi::print_str("\n");
    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}
