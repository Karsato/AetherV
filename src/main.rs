#![no_std]
#![no_main]

mod entry;
mod sbi;
mod trap;
mod paging;
mod task;
mod fdt;
mod drivers;

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
    
    // Analizar el Device Tree (FDT) proporcionado por OpenSBI en _fdt_ptr (a1)
    unsafe {
        fdt::parse_fdt(_fdt_ptr);
    }

    // Inicializar el sistema de trampas (Trap Handler)
    trap::init();
    sbi::print_str("[Kernel] Sistema de trampas inicializado.\n");

    // Inicializar memoria virtual (Paginación Sv39)
    paging::init();

    // Inicializar controlador gráfico VirtIO GPU
    drivers::gpu::init();

    // Activar interrupción del temporizador
    trap::enable_timer_interrupt();
    sbi::print_str("[Kernel] Interrupciones de reloj activadas.\n");

    // Prueba 1: realizar un ebreak (breakpoint) en S-mode para verificar
    sbi::print_str("[Kernel] Probando ebreak (breakpoint) en S-mode...\n");
    unsafe {
        core::arch::asm!("ebreak");
    }
    sbi::print_str("[Kernel] Retorno de ebreak exitoso!\n");

    // Calcular dirección del entry point del usuario usando el mapeo virtual U-mode (offset -0x40000000)
    let user_entry_va = (user_task as *const () as usize) - 0x40000000;
    // Crear y registrar tareas secundarias (Tarea 1 en Modo Usuario, Tarea 2 en Modo Supervisor)
    task::create_user_task(1, user_entry_va);
    task::create_task(2, task2);
    sbi::print_str("[Kernel] Tareas concurrentes 1 (User) y 2 (Kernel) creadas.\n");
    sbi::print_str("[Kernel] Iniciando planificador multitarea...\n");

    let mut count = 0;
    loop {
        sbi::print_str("M");
        count += 1;
        if count == 100 {
            sbi::print_str("\n[Main Thread] Cediendo CPU de forma cooperativa...\n");
            task::yield_cpu();
            count = 0;
        }
        for _ in 0..200000 {
            unsafe { core::arch::asm!("nop"); }
        }
    }
}

fn user_print(s: &str) {
    let ptr = s.as_ptr() as usize;
    let len = s.len();
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 3,
            in("a0") ptr,
            in("a1") len,
            lateout("a0") _
        );
    }
}

fn user_task() {
    let mut count = 0;
    loop {
        user_print("[U-Mode] Hola desde la tarea de espacio de usuario!\n");

        // Llamada al sistema sys_yield (syscall_id = 1)
        unsafe {
            core::arch::asm!(
                "ecall",
                in("a7") 1
            );
        }

        count += 1;
        if count == 5 {
            user_print("[U-Mode] Tarea finalizada, llamando a sys_exit...\n");
            // Llamada al sistema sys_exit (syscall_id = 2)
            unsafe {
                core::arch::asm!(
                    "ecall",
                    in("a7") 2
                );
            }
        }
        for _ in 0..200000 {
            unsafe { core::arch::asm!("nop"); }
        }
    }
}

fn task2() {
    let mut count = 0;
    loop {
        sbi::print_str("B");
        count += 1;
        if count == 120 {
            sbi::print_str("\n[Task 2] Cediendo CPU de forma cooperativa...\n");
            task::yield_cpu();
            count = 0;
        }
        for _ in 0..200000 {
            unsafe { core::arch::asm!("nop"); }
        }
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
