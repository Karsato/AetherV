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
    let user_entry_va1 = (gpu_client as *const () as usize) - 0x40000000;
    let user_entry_va2 = (gpu_driver_server as *const () as usize) - 0x40000000;
    // Crear y registrar tareas secundarias (Tarea 1 y Tarea 2 en Modo Usuario)
    task::create_user_task(1, user_entry_va1);
    task::create_user_task(2, user_entry_va2);
    sbi::print_str("[Kernel] Tareas de usuario 1 (Cliente) y 2 (Servidor) creadas.\n");
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
            inout("a0") ptr => _,
            in("a1") len,
            clobber_abi("C"),
        );
    }
}

fn gpu_client() {
    user_print("[GPU Client] Iniciado. Solicitando al Servidor GPU rellenar la pantalla de azul...\n");

    let mut msg = crate::task::IpcMessage {
        sender: 0,
        msg_type: 2, // Comando de color sólido
        length: 8,
        reserved: 0,
        payload: [0; 32],
    };
    // Color azul: R=0, G=0, B=255
    msg.payload[0] = 0;   // R
    msg.payload[1] = 0;   // G
    msg.payload[2] = 255; // B

    let mut res: isize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 4, // sys_ipc_send
            inout("a0") 2_isize => res, // Dest: Tarea 2 (GPU Server)
            in("a1") &msg as *const _ as usize,
            clobber_abi("C"),
        );
    }

    if res == 0 {
        user_print("[GPU Client] Petición enviada. Esperando confirmación...\n");
        #[allow(unused_mut)]
        let mut reply = crate::task::IpcMessage {
            sender: 0,
            msg_type: 0,
            length: 0,
            reserved: 0,
            payload: [0; 32],
        };
        unsafe {
            core::arch::asm!(
                "ecall",
                in("a7") 5, // sys_ipc_recv
                inout("a0") 2_isize => res, // Src: Tarea 2
                in("a1") &reply as *const _ as usize,
                clobber_abi("C"),
            );
        }
        if res == 0 && reply.msg_type == 200 {
            user_print("[GPU Client] Pantalla azul pintada con éxito!\n");
        } else {
            user_print("[GPU Client] Error en la ejecución del comando gráfico.\n");
        }
    } else {
        user_print("[GPU Client] Error al enviar comando.\n");
    }

    // Esperar un poco y luego pintar degradado cromático original
    for _ in 0..4000000 {
        unsafe { core::arch::asm!("nop"); }
    }

    user_print("[GPU Client] Solicitando al Servidor GPU restaurar el patrón degradado cromático...\n");
    msg.msg_type = 1; // Comando de degradado cromático

    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 4, // sys_ipc_send
            inout("a0") 2_isize => res, // Dest: Tarea 2 (GPU Server)
            in("a1") &msg as *const _ as usize,
            clobber_abi("C"),
        );
    }

    if res == 0 {
        user_print("[GPU Client] Petición enviada. Esperando confirmación...\n");
        #[allow(unused_mut)]
        let mut reply2 = crate::task::IpcMessage {
            sender: 0,
            msg_type: 0,
            length: 0,
            reserved: 0,
            payload: [0; 32],
        };
        unsafe {
            core::arch::asm!(
                "ecall",
                in("a7") 5, // sys_ipc_recv
                inout("a0") 2_isize => res, // Src: Tarea 2
                in("a1") &reply2 as *const _ as usize,
                clobber_abi("C"),
            );
        }
        if res == 0 && reply2.msg_type == 200 {
            user_print("[GPU Client] Patrón degradado cromático restaurado con éxito!\n");
        }
    }

    user_print("[GPU Client] Tarea finalizada limpiamente.\n");
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 2, // sys_exit
            clobber_abi("C"),
        );
    }
}

fn gpu_driver_server() {
    user_print("[GPU Server] Iniciando inicialización en U-Mode...\n");
    drivers::gpu::init();
    user_print("[GPU Server] Inicialización completada con éxito. Entrando en bucle de servicio IPC...\n");

    loop {
        #[allow(unused_mut)]
        let mut msg = crate::task::IpcMessage {
            sender: 0,
            msg_type: 0,
            length: 0,
            reserved: 0,
            payload: [0; 32],
        };
        let mut res: isize;
        unsafe {
            core::arch::asm!(
                "ecall",
                in("a7") 5, // sys_ipc_recv
                inout("a0") crate::task::IPC_WILDCARD => res, // Src: cualquiera
                in("a1") &msg as *const _ as usize,
                clobber_abi("C"),
            );
        }

        if res == 0 {
            user_print("[GPU Server] Solicitud recibida!\n");
            match msg.msg_type {
                1 => {
                    user_print("[GPU Server] Comando de dibujo: draw_pattern\n");
                    drivers::gpu::draw_pattern();
                    drivers::gpu::flush_screen(0x10008000);
                    msg.msg_type = 200; // Éxito
                }
                2 => {
                    let r = msg.payload[0] as u32;
                    let g = msg.payload[1] as u32;
                    let b = msg.payload[2] as u32;
                    user_print("[GPU Server] Comando de dibujo: rellenar color sólido\n");
                    unsafe {
                        let fb = &mut drivers::gpu::FRAMEBUFFER;
                        let color_val = 0xFF000000 | (r << 16) | (g << 8) | b;
                        for pixel in fb.pixels.iter_mut() {
                            *pixel = color_val;
                        }
                    }
                    drivers::gpu::flush_screen(0x10008000);
                    msg.msg_type = 200; // Éxito
                }
                _ => {
                    user_print("[GPU Server] Comando desconocido\n");
                    msg.msg_type = 404; // Desconocido
                }
            }

            // Responder al cliente
            unsafe {
                core::arch::asm!(
                    "ecall",
                    in("a7") 4, // sys_ipc_send
                    inout("a0") msg.sender as usize => _, // Dest: cliente
                    in("a1") &msg as *const _ as usize,
                    clobber_abi("C"),
                );
            }
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
