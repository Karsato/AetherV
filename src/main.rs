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
    let user_entry_va1 = (user_task as *const () as usize) - 0x40000000;
    let user_entry_va2 = (user_task2 as *const () as usize) - 0x40000000;
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
            in("a0") ptr,
            in("a1") len,
            lateout("a0") _
        );
    }
}

fn user_task() {
    let mut msg = crate::task::IpcMessage {
        sender: 0,
        msg_type: 100, // Tipo petición
        length: 8,
        reserved: 0,
        payload: [0; 32],
    };
    msg.payload[0] = 0xDE;
    msg.payload[1] = 0xAD;
    msg.payload[2] = 0xBE;
    msg.payload[3] = 0xEF;

    user_print("[Client] Iniciado. Enviando mensaje de peticion a Server (Tarea 2)...\n");
    
    let mut res: isize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 4, // sys_ipc_send
            in("a0") 2, // Dest: Tarea 2
            in("a1") &msg as *const _ as usize,
            lateout("a0") res
        );
    }
    
    if res == 0 {
        user_print("[Client] Mensaje enviado! Esperando respuesta de Server...\n");
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
                in("a0") 2, // Src: Tarea 2
                in("a1") &reply as *const _ as usize,
                lateout("a0") res
            );
        }
        if res == 0 {
            user_print("[Client] Respuesta recibida de Server con exito!\n");
            if reply.payload[0] == 0xCA && reply.payload[1] == 0xFE {
                user_print("[Client] Servidor respondio correctamente con CAFE!\n");
            }
        } else {
            user_print("[Client] Error al recibir respuesta.\n");
        }
    } else {
        user_print("[Client] Error al enviar mensaje.\n");
    }

    user_print("[Client] Tarea finalizada, llamando a sys_exit...\n");
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 2 // sys_exit
        );
    }
}

fn user_task2() {
    #[allow(unused_mut)]
    let mut req = crate::task::IpcMessage {
        sender: 0,
        msg_type: 0,
        length: 0,
        reserved: 0,
        payload: [0; 32],
    };
    
    user_print("[Server] Iniciado. Esperando peticion de Client (Tarea 1)...\n");
    
    let mut res: isize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 5, // sys_ipc_recv
            in("a0") 1, // Src: Tarea 1
            in("a1") &req as *const _ as usize,
            lateout("a0") res
        );
    }
    
    if res == 0 {
        user_print("[Server] Peticion recibida de Client!\n");
        if req.payload[0] == 0xDE && req.payload[1] == 0xAD {
            user_print("[Server] Cliente envio DEADBEEF!\n");
        }
        
        let mut reply = crate::task::IpcMessage {
            sender: 0,
            msg_type: 200, // Tipo respuesta
            length: 8,
            reserved: 0,
            payload: [0; 32],
        };
        reply.payload[0] = 0xCA;
        reply.payload[1] = 0xFE;
        reply.payload[2] = 0xBA;
        reply.payload[3] = 0xBE;
        
        user_print("[Server] Enviando respuesta a Client...\n");
        unsafe {
            core::arch::asm!(
                "ecall",
                in("a7") 4, // sys_ipc_send
                in("a0") 1, // Dest: Tarea 1
                in("a1") &reply as *const _ as usize,
                lateout("a0") res
            );
        }
        if res == 0 {
            user_print("[Server] Respuesta enviada con exito!\n");
        }
    } else {
        user_print("[Server] Error al recibir peticion.\n");
    }
    
    user_print("[Server] Tarea finalizada, llamando a sys_exit...\n");
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 2 // sys_exit
        );
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
