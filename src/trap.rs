
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
    pub const TIMER_INTERVAL: u64 = 1_000_000;

    #[no_mangle]
    pub extern "C" fn rust_trap_handler(tf: &mut TrapFrame) {
        // Guardar la dirección del TrapFrame actual de la tarea activa solo si viene de U-mode
        unsafe {
            let spp = (tf.sstatus >> 8) & 1;
            if spp == 0 {
                crate::task::SCHEDULER.tasks[crate::task::SCHEDULER.current_id].tf_addr = tf as *mut _ as usize;
            }
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
                9 => { // Supervisor External Interrupt (SEI)
                    handle_external_interrupt();
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
                        1 => { // sys_putchar (para que panics de U-mode impriman en consola)
                            let c = tf.regs[10];
                            sbi::sbi_putchar(c);
                            tf.regs[10] = 0;
                            tf.sepc += 4;
                        }
                        10 => { // sys_yield
                            tf.sepc += 4;
                            crate::task::yield_cpu();
                        }
                        2 => { // sys_exit
                            tf.sepc += 4;
                            unsafe {
                                crate::task::SCHEDULER.exit_current_task();
                            }
                        }
                        3 => { // sys_write (fd, ptr, len)
                            let fd = tf.regs[10];               // a0: FD (1=stdout, 2=stderr, 3=info, 4=debug)
                            let ptr = tf.regs[11] as *const u8; // a1: buffer ptr
                            let len = tf.regs[12];              // a2: length
                            let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
                            if let Ok(s) = core::str::from_utf8(slice) {
                                let current_level = crate::task::get_log_level();
                                match fd {
                                    1 => sbi::print_str(s), // stdout (Shell): siempre visible
                                    2 if current_level >= 1 => sbi::print_str(s), // stderr / log_error
                                    3 if current_level >= 2 => sbi::print_str(s), // log_info
                                    4 if current_level >= 3 => sbi::print_str(s), // log_debug
                                    _ => {} // Silenciado por LogLevel
                                }
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
        if crate::task::is_debug_enabled() {
            sbi::print_str(".");
        }
        sbi::sbi_set_timer(sbi::get_time() + TIMER_INTERVAL);
        unsafe { crate::task::SCHEDULER.schedule(); }
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

    fn handle_external_interrupt() {
        unsafe {
            // Claim de la interrupción en el PLIC Contexto 1 (Hart 0 S-mode, registro Claim en 0x0c20_1004)
            let claim_ptr = 0x0c20_1004 as *mut u32;
            let irq = core::ptr::read_volatile(claim_ptr);

            if irq != 0 {
                if crate::task::is_debug_enabled() { sbi::print_str("[Trap] External Interrupt claimed: ");
                sbi::print_hex(irq as usize);
                sbi::print_str("\n"); }

                // Si es IRQ 6 o 7 (Dispositivos VirtIO Input), notificar al Servidor de Entrada (Tarea 4)
                if irq == 6 || irq == 7 {
                    let base = 0x10001000 + ((irq - 1) as usize) * 0x1000;
                    let int_status = core::ptr::read_volatile((base + 0x60) as *const u32);
                    core::ptr::write_volatile((base + 0x64) as *mut u32, int_status & 0x3);

                    crate::task::sys_ipc_notify(4, 1);
                }

                // Completar la interrupción escribiendo el IRQ de vuelta al registro Complete
                core::ptr::write_volatile(claim_ptr, irq);
            }
        }
    }

    pub fn plic_init() {
        unsafe {
            let thresh_ptr = 0x0c20_1000 as *mut u32;
            sbi::print_str("[PLIC] Threshold antes: ");
            sbi::print_hex(core::ptr::read_volatile(thresh_ptr) as usize);
            core::ptr::write_volatile(thresh_ptr, 0);
            sbi::print_str(", despues: ");
            sbi::print_hex(core::ptr::read_volatile(thresh_ptr) as usize);
            sbi::print_str("\n");
        }
    }

    pub fn plic_enable_irq(irq: u32) {
        unsafe {
            // Establecer prioridad del IRQ a 1 (no cero)
            let priority_ptr = (0x0c00_0000 + (irq as usize) * 4) as *mut u32;
            sbi::print_str("[PLIC] Prioridad IRQ ");
            sbi::print_hex(irq as usize);
            sbi::print_str(" antes: ");
            sbi::print_hex(core::ptr::read_volatile(priority_ptr) as usize);
            core::ptr::write_volatile(priority_ptr, 1);
            sbi::print_str(", despues: ");
            sbi::print_hex(core::ptr::read_volatile(priority_ptr) as usize);
            sbi::print_str("\n");

            // Habilitar el IRQ en el registro Enable del PLIC para Hart 0 S-mode (Contexto 1)
            let enable_ptr = (0x0c00_2080 + ((irq as usize) / 32) * 4) as *mut u32;
            let mask = 1u32 << (irq % 32);
            sbi::print_str("[PLIC] Enable antes: ");
            sbi::print_hex(core::ptr::read_volatile(enable_ptr) as usize);
            let val = core::ptr::read_volatile(enable_ptr);
            core::ptr::write_volatile(enable_ptr, val | mask);
            sbi::print_str(", despues: ");
            sbi::print_hex(core::ptr::read_volatile(enable_ptr) as usize);
            sbi::print_str("\n");
        }
    }

    pub fn enable_external_interrupt() {
        unsafe {
            // Habilitar Supervisor External Interrupts en sie (bit 9 es SEIE)
            core::arch::asm!("csrs sie, {}", in(reg) (1 << 9));
        }
    }
