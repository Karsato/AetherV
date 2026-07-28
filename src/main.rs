#![no_std]
#![no_main]
#![allow(static_mut_refs)]

mod entry;
mod sbi;
mod trap;
mod paging;
mod task;
mod fdt;
mod drivers;
mod apps;

use core::panic::PanicInfo;
use apps::{
    nameserver::nameserver_task,
    gpu_server::gpu_driver_server,
    input_server::input_driver_server,
    wm::window_manager_task,
    vfs_server::vfs_server,
    shell::shell_task,
    clients::{window_client_1, window_client_2, vfs_client},
};

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

    unsafe { fdt::parse_fdt(_fdt_ptr); }

    trap::init();
    sbi::print_str("[Kernel] Sistema de trampas inicializado.\n");

    paging::init();

    trap::enable_timer_interrupt();
    sbi::print_str("[Kernel] Interrupciones de reloj activadas.\n");

    trap::plic_init();
    trap::plic_enable_irq(6); // Mouse
    trap::plic_enable_irq(7); // Keyboard
    trap::enable_external_interrupt();
    sbi::print_str("[Kernel] Interrupciones de PLIC para VirtIO Input (IRQ 6 y 7) activadas.\n");

    sbi::print_str("[Kernel] Probando ebreak (breakpoint) en S-mode...\n");
    unsafe { core::arch::asm!("ebreak"); }
    sbi::print_str("[Kernel] Retorno de ebreak exitoso!\n");

    // Calcular VAs de entrada U-Mode (offset -0x40000000)
    let user_entry_va1 = (gpu_driver_server     as *const () as usize) - 0x40000000;
    let user_entry_va2 = (window_manager_task   as *const () as usize) - 0x40000000;
    let user_entry_va3 = (nameserver_task        as *const () as usize) - 0x40000000;
    let user_entry_va4 = (input_driver_server   as *const () as usize) - 0x40000000;
    let user_entry_va5 = (window_client_1       as *const () as usize) - 0x40000000;
    let user_entry_va6 = (window_client_2       as *const () as usize) - 0x40000000;
    let user_entry_va7 = (vfs_server            as *const () as usize) - 0x40000000;
    let user_entry_va8 = (vfs_client            as *const () as usize) - 0x40000000;
    let user_entry_va9 = (shell_task            as *const () as usize) - 0x40000000;

    task::create_user_task(1, user_entry_va1); // GPU Server
    task::create_user_task(2, user_entry_va2); // Window Manager
    task::create_user_task(3, user_entry_va3); // Nameserver
    task::create_user_task(4, user_entry_va4); // Input Driver
    task::create_user_task(5, user_entry_va5); // Client 1
    task::create_user_task(6, user_entry_va6); // Client 2
    task::create_user_task(7, user_entry_va7); // VFS Server
    task::create_user_task(8, user_entry_va8); // VFS Client (pasivo)
    task::create_user_task(9, user_entry_va9); // Shell
    sbi::print_str("[Kernel] Tareas de usuario 1 a 9 creadas.\n");
    sbi::print_str("[Kernel] Iniciando planificador multitarea...\n");

    // Warm-up cooperativo → luego WFI
    let mut count = 0;
    let mut loops = 0;
    loop {
        sbi::print_str("M");
        count += 1;
        if count == 100 {
            sbi::print_str("\n[Main Thread] Cediendo CPU de forma cooperativa...\n");
            task::yield_cpu();
            count = 0;
            loops += 1;
            if loops == 3 {
                apps::log_info("\n[Main Thread] Entrando en estado de reposo de bajo consumo (WFI)...\n");
                break;
            }
        }
        for _ in 0..200000 {
            unsafe { core::arch::asm!("nop"); }
        }
    }

    // Bucle de reposo S-Mode
    loop {
        sbi::sbi_set_timer(sbi::get_time() + crate::trap::TIMER_INTERVAL);
        unsafe { core::arch::asm!("wfi"); }
        task::yield_cpu();
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
