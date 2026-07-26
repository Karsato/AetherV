#![allow(static_mut_refs)]
// Manejo de excepciones e interrupciones en Supervisor Mode
use crate::sbi;

core::arch::global_asm!(include_str!("trap.S"));

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TrapFrame {
    pub regs: [usize; 32], // x0 a x31
    pub sstatus: usize,
    pub sepc: usize,
}

// Intervalo de tiempo para la simulación del reloj (ajustado para QEMU)
const TIMER_INTERVAL: u64 = 1_000_000;

#[no_mangle]
pub extern "C" fn rust_trap_handler(tf: &mut TrapFrame) {
    // Guardar la dirección del TrapFrame actual de la tarea activa
    unsafe {
        crate::task::SCHEDULER.tasks[crate::task::SCHEDULER.current_id].tf_addr = tf as *mut _ as usize;
    }
    let scause: usize;
    let stval: usize;
    unsafe {
        core::arch::asm!("csrr {}, scause", out(reg) scause);
        core::arch::asm!("csrr {}, stval", out(reg) stval);
    }

    let is_interrupt = (scause >> 63) != 0;
    let code = scause & !(1 << 63);

    if is_interrupt {
        match code {
            5 => { // Supervisor Timer Interrupt (STI)
                handle_timer_interrupt();
            }
            _ => {
                sbi::print_str("\n[Trap] Interrupción no controlada: ");
                sbi::print_hex(code);
                sbi::print_str("\n");
            }
        }
    } else {
        match code {
            3 => { // Breakpoint / ebreak
                sbi::print_str("\n[Breakpoint] ebreak en S-mode capturado con éxito!\n");
                sbi::print_str("sepc: ");
                sbi::print_hex(tf.sepc);
                sbi::print_str("\n");
                // Avanzar sepc según la longitud de la instrucción ebreak (comprimida o de 32 bits)
                let inst = unsafe { *(tf.sepc as *const u16) };
                let len = if (inst & 0x3) == 0x3 { 4 } else { 2 };
                tf.sepc += len;
            }
            8 => { // Environment Call desde U-mode
                let syscall_id = tf.regs[17]; // a7 es x17
                match syscall_id {
                    1 => { // sys_yield
                        tf.sepc += 4;
                        crate::task::yield_cpu();
                    }
                    2 => { // sys_exit
                        tf.sepc += 4;
                        unsafe {
                            crate::task::SCHEDULER.exit_current_task();
                        }
                    }
                    3 => { // sys_write (para imprimir desde U-mode)
                        let ptr = tf.regs[10] as *const u8; // a0
                        let len = tf.regs[11]; // a1
                        let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
                        if let Ok(s) = core::str::from_utf8(slice) {
                            sbi::print_str(s);
                        }
                        tf.regs[10] = 0; // Retornar 0 (éxito) en a0
                        tf.sepc += 4;
                    }
                    4 => { // sys_ipc_send
                        let dest_id = tf.regs[10];
                        let msg_ptr = tf.regs[11];
                        let res = crate::task::sys_ipc_send(dest_id, msg_ptr);
                        tf.regs[10] = res as usize;
                        tf.sepc += 4;
                    }
                    5 => { // sys_ipc_recv
                        let src_id = tf.regs[10];
                        let msg_ptr = tf.regs[11];
                        let res = crate::task::sys_ipc_recv(src_id, msg_ptr);
                        tf.regs[10] = res as usize;
                        tf.sepc += 4;
                    }
                    6 => { // sys_ipc_reply_recv
                        let dest_id = tf.regs[10];
                        let reply_msg_ptr = tf.regs[11];
                        let src_id = tf.regs[12];
                        let recv_msg_ptr = tf.regs[13];
                        let res = crate::task::sys_ipc_reply_recv(dest_id, reply_msg_ptr, src_id, recv_msg_ptr);
                        tf.regs[10] = res as usize;
                        tf.sepc += 4;
                    }
                    7 => { // sys_ipc_notify
                        let dest_id = tf.regs[10];
                        let bits = tf.regs[11] as u32;
                        let res = crate::task::sys_ipc_notify(dest_id, bits);
                        tf.regs[10] = res as usize;
                        tf.sepc += 4;
                    }
                    _ => {
                        sbi::print_str("\n[Syscall] ecall desde U-mode no implementado: ");
                        sbi::print_hex(syscall_id);
                        sbi::print_str("\n");
                        tf.sepc += 4;
                    }
                }
            }
            9 => { // Environment Call desde S-mode
                let syscall_id = tf.regs[17]; // a7 es x17
                match syscall_id {
                    1 => { // sys_yield
                        tf.sepc += 4;
                        crate::task::yield_cpu();
                    }
                    2 => { // sys_exit
                        unsafe {
                            crate::task::SCHEDULER.exit_current_task();
                        }
                    }
                    _ => {
                        sbi::print_str("\n[Syscall] ecall desde S-mode capturado con éxito.\n");
                        tf.sepc += 4; // Continuar tras la instrucción ecall
                    }
                }
            }
            2 => {
                sbi::print_str("\n[Exception] Instrucción ilegal detectada!\n");
                sbi::print_str("sepc: ");
                sbi::print_hex(tf.sepc);
                sbi::print_str("\n");
                panic!("Ejecución detenida por excepción de hardware.");
            }
            12 | 13 | 15 => {
                sbi::print_str("\n[Exception] Fallo de página detectado (código ");
                sbi::print_hex(code);
                sbi::print_str(")\n");
                sbi::print_str("Dirección de fallo (stval): ");
                sbi::print_hex(stval);
                sbi::print_str("\n");
                sbi::print_str("sepc: ");
                sbi::print_hex(tf.sepc);
                sbi::print_str("\n");
                panic!("Ejecución detenida por fallo de página.");
            }
            _ => {
                sbi::print_str("\n[Exception] Excepción fatal: ");
                sbi::print_hex(code);
                sbi::print_str(", stval: ");
                sbi::print_hex(stval);
                sbi::print_str(", sepc: ");
                sbi::print_hex(tf.sepc);
                sbi::print_str("\n");
                panic!("Excepción fatal no controlada.");
            }
        }
    }
}

fn handle_timer_interrupt() {
    // Imprimir un punto en la consola serial para denotar el tick del timer
    sbi::print_str(".");
    // Programar la siguiente interrupción de temporizador
    sbi::sbi_set_timer(sbi::get_time() + TIMER_INTERVAL);
    // Cambiar preventivamente de tarea (Preemptive context switch)
    unsafe {
        crate::task::SCHEDULER.schedule();
    }
}

pub fn init() {
    extern "C" {
        fn trap_entry();
    }
    unsafe {
        // Inicializar sscratch a 0 para el modo S-mode
        core::arch::asm!("csrw sscratch, zero");
        // Modo Directo: stvec = trap_entry (los bits de modo son 00)
        let trap_entry_addr = trap_entry as *const () as usize;
        core::arch::asm!("csrw stvec, {}", in(reg) trap_entry_addr);
    }
}

pub fn enable_timer_interrupt() {
    unsafe {
        // Habilitar Timer Interrupts en sie (Supervisor Interrupt Enable, bit 5 es STIE)
        core::arch::asm!("csrs sie, {}", in(reg) (1 << 5));
        // Habilitar interrupciones globales en sstatus (bit 1 es SIE) y habilitar SUM (bit 18)
        core::arch::asm!("csrs sstatus, {}", in(reg) ((1 << 1) | (1 << 18)));
    }
    // Programar el primer tick del temporizador
    sbi::sbi_set_timer(sbi::get_time() + TIMER_INTERVAL);
}
