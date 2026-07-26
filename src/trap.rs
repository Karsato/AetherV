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
        // Modo Directo: stvec = trap_entry (los bits de modo son 00)
        let trap_entry_addr = trap_entry as *const () as usize;
        core::arch::asm!("csrw stvec, {}", in(reg) trap_entry_addr);
    }
}

pub fn enable_timer_interrupt() {
    unsafe {
        // Habilitar Timer Interrupts en sie (Supervisor Interrupt Enable, bit 5 es STIE)
        core::arch::asm!("csrs sie, {}", in(reg) (1 << 5));
        // Habilitar interrupciones globales en sstatus (bit 1 es SIE)
        core::arch::asm!("csrs sstatus, {}", in(reg) (1 << 1));
    }
    // Programar el primer tick del temporizador
    sbi::sbi_set_timer(sbi::get_time() + TIMER_INTERVAL);
}
