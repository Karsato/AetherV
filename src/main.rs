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

    // Inicializar PLIC y habilitar interrupción externa para VirtIO Input (IRQ 6 y 7)
    trap::plic_init();
    trap::plic_enable_irq(6); // Mouse
    trap::plic_enable_irq(7); // Keyboard
    trap::enable_external_interrupt();
    sbi::print_str("[Kernel] Interrupciones de PLIC para VirtIO Input (IRQ 6 y 7) activadas.\n");

    // Prueba 1: realizar un ebreak (breakpoint) en S-mode para verificar
    sbi::print_str("[Kernel] Probando ebreak (breakpoint) en S-mode...\n");
    unsafe {
        core::arch::asm!("ebreak");
    }
    sbi::print_str("[Kernel] Retorno de ebreak exitoso!\n");

    // Calcular dirección del entry point del usuario usando el mapeo virtual U-mode (offset -0x40000000)
    let user_entry_va1 = (gpu_driver_server as *const () as usize) - 0x40000000;
    let user_entry_va2 = (gpu_client as *const () as usize) - 0x40000000;
    let user_entry_va3 = (nameserver_task as *const () as usize) - 0x40000000;
    let user_entry_va4 = (input_driver_server as *const () as usize) - 0x40000000;
    let user_entry_va5 = (input_client as *const () as usize) - 0x40000000;
    // Crear y registrar tareas secundarias (Tareas en Modo Usuario)
    task::create_user_task(1, user_entry_va1); // Tarea 1: Servidor GPU
    task::create_user_task(2, user_entry_va2); // Tarea 2: Cliente
    task::create_user_task(3, user_entry_va3); // Tarea 3: Nameserver
    task::create_user_task(4, user_entry_va4); // Tarea 4: Servidor Input
    task::create_user_task(5, user_entry_va5); // Tarea 5: Cliente Input
    sbi::print_str("[Kernel] Tareas de usuario 1 a 5 creadas.\n");
    sbi::print_str("[Kernel] Iniciando planificador multitarea...\n");

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
                sbi::print_str("\n[Main Thread] Entrando en estado de reposo de bajo consumo (WFI)...\n");
                break;
            }
        }
        for _ in 0..200000 {
            unsafe { core::arch::asm!("nop"); }
        }
    }

    // Bucle de reposo de bajo consumo en S-Mode
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
        task::yield_cpu();
    }
}

// Códigos de comando del Nameserver
pub const NS_CMD_REGISTER: u32 = 1001;
pub const NS_CMD_LOOKUP:   u32 = 1002;
pub const NS_RESP_SUCCESS: u32 = 2000;
pub const NS_RESP_ERROR:   u32 = 4000;

// Códigos de comando del Driver de Teclado
pub const INPUT_CMD_GET_KEY: u32 = 2001;
pub const INPUT_RESP_KEY:     u32 = 2002;
pub const INPUT_RESP_EMPTY:   u32 = 2003;

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

fn user_ipc_send(dest: usize, msg: &crate::task::IpcMessage) -> isize {
    let mut res: isize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 4, // sys_ipc_send
            inout("a0") dest as isize => res,
            in("a1") msg as *const _ as usize,
            clobber_abi("C"),
        );
    }
    res
}

fn user_ipc_recv(src: usize, msg: &mut crate::task::IpcMessage) -> isize {
    let mut res: isize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 5, // sys_ipc_recv
            inout("a0") src as isize => res,
            in("a1") msg as *mut _ as usize,
            clobber_abi("C"),
        );
    }
    res
}

fn str_to_u8_16(s: &str) -> [u8; 16] {
    let mut arr = [0u8; 16];
    let bytes = s.as_bytes();
    let len = core::cmp::min(bytes.len(), 16);
    arr[0..len].copy_from_slice(&bytes[0..len]);
    arr
}

#[derive(Copy, Clone, Debug)]
struct ServiceEntry {
    name: [u8; 16],
    task_id: usize,
}

static mut SERVICES: [Option<ServiceEntry>; 8] = [None; 8];

fn nameserver_task() {
    user_print("[Nameserver] Inicializando servidor de nombres en U-Mode...\n");
    loop {
        let mut msg = crate::task::IpcMessage {
            sender: 0,
            msg_type: 0,
            length: 0,
            reserved: 0,
            payload: [0; 32],
        };
        let res = user_ipc_recv(crate::task::IPC_WILDCARD, &mut msg);
        if res == 0 {
            user_print("[Nameserver] Solicitud recibida!\n");
            match msg.msg_type {
                NS_CMD_REGISTER => {
                    let mut name = [0u8; 16];
                    name.copy_from_slice(&msg.payload[0..16]);
                    let task_id = msg.sender as usize;
                    
                    user_print("[Nameserver] Comando: Registrar servicio\n");
                    
                    let mut registered = false;
                    unsafe {
                        for entry in SERVICES.iter_mut() {
                            if let Some(e) = entry {
                                if e.name == name {
                                    e.task_id = task_id;
                                    registered = true;
                                    break;
                                }
                            }
                        }
                        if !registered {
                            for entry in SERVICES.iter_mut() {
                                if entry.is_none() {
                                    *entry = Some(ServiceEntry { name, task_id });
                                    registered = true;
                                    break;
                                }
                            }
                        }
                    }
                    
                    let reply = crate::task::IpcMessage {
                        sender: 3,
                        msg_type: if registered { NS_RESP_SUCCESS } else { NS_RESP_ERROR },
                        length: 0,
                        reserved: 0,
                        payload: [0; 32],
                    };
                    user_ipc_send(task_id, &reply);
                }
                NS_CMD_LOOKUP => {
                    let mut name = [0u8; 16];
                    name.copy_from_slice(&msg.payload[0..16]);
                    
                    user_print("[Nameserver] Comando: Resolver servicio\n");
                    
                    let mut found_id = None;
                    unsafe {
                        for entry in SERVICES.iter() {
                            if let Some(e) = entry {
                                if e.name == name {
                                    found_id = Some(e.task_id);
                                    break;
                                }
                            }
                        }
                    }
                    
                    let mut reply = crate::task::IpcMessage {
                        sender: 3,
                        msg_type: if found_id.is_some() { NS_RESP_SUCCESS } else { NS_RESP_ERROR },
                        length: 4,
                        reserved: 0,
                        payload: [0; 32],
                    };
                    if let Some(tid) = found_id {
                        let bytes = (tid as u32).to_ne_bytes();
                        reply.payload[16..20].copy_from_slice(&bytes);
                    }
                    user_ipc_send(msg.sender as usize, &reply);
                }
                _ => {
                    user_print("[Nameserver] Comando desconocido\n");
                    let reply = crate::task::IpcMessage {
                        sender: 3,
                        msg_type: NS_RESP_ERROR,
                        length: 0,
                        reserved: 0,
                        payload: [0; 32],
                    };
                    user_ipc_send(msg.sender as usize, &reply);
                }
            }
        }
    }
}

fn gpu_client() {
    // Buscar el servicio "display" en el Nameserver (Tarea 3)
    user_print("[GPU Client] Buscando el servicio 'display' en el Nameserver (Tarea 3)...\n");
    let mut lookup_msg = crate::task::IpcMessage {
        sender: 0,
        msg_type: NS_CMD_LOOKUP,
        length: 16,
        reserved: 0,
        payload: [0; 32],
    };
    lookup_msg.payload[0..16].copy_from_slice(&str_to_u8_16("display"));

    let mut gpu_task_id = 0;
    let mut res = user_ipc_send(3, &lookup_msg);
    if res == 0 {
        let mut reply = crate::task::IpcMessage {
            sender: 0,
            msg_type: 0,
            length: 0,
            reserved: 0,
            payload: [0; 32],
        };
        res = user_ipc_recv(3, &mut reply);
        if res == 0 && reply.msg_type == NS_RESP_SUCCESS {
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&reply.payload[16..20]);
            gpu_task_id = u32::from_ne_bytes(bytes) as usize;
            user_print("[GPU Client] Servicio 'display' resuelto con éxito.\n");
        } else {
            user_print("[GPU Client] Error al resolver el servicio 'display'.\n");
            unsafe {
                core::arch::asm!("ecall", in("a7") 2, clobber_abi("C"));
            }
        }
    } else {
        user_print("[GPU Client] Error al conectar con el Nameserver.\n");
        unsafe {
            core::arch::asm!("ecall", in("a7") 2, clobber_abi("C"));
        }
    }

    user_print("[GPU Client] Solicitando al Servidor GPU rellenar la pantalla de azul...\n");

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

    res = user_ipc_send(gpu_task_id, &msg);

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
        res = user_ipc_recv(gpu_task_id, &mut reply);
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

    res = user_ipc_send(gpu_task_id, &msg);

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
        res = user_ipc_recv(gpu_task_id, &mut reply2);
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
    user_print("[GPU Server] Inicialización completada con éxito.\n");

    // Registrar el servicio "display" en el Nameserver (Tarea 3)
    user_print("[GPU Server] Registrando servicio 'display' en el Nameserver (Tarea 3)...\n");
    let mut reg_msg = crate::task::IpcMessage {
        sender: 0,
        msg_type: NS_CMD_REGISTER,
        length: 16,
        reserved: 0,
        payload: [0; 32],
    };
    reg_msg.payload[0..16].copy_from_slice(&str_to_u8_16("display"));
    
    let mut res = user_ipc_send(3, &reg_msg);
    if res == 0 {
        let mut reply = crate::task::IpcMessage {
            sender: 0,
            msg_type: 0,
            length: 0,
            reserved: 0,
            payload: [0; 32],
        };
        res = user_ipc_recv(3, &mut reply);
        if res == 0 && reply.msg_type == NS_RESP_SUCCESS {
            user_print("[GPU Server] Registro exitoso en el Nameserver!\n");
        } else {
            user_print("[GPU Server] Error en el registro en el Nameserver.\n");
        }
    } else {
        user_print("[GPU Server] Error al conectar con el Nameserver.\n");
    }

    user_print("[GPU Server] Entrando en bucle de servicio IPC...\n");

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

static mut KEY_BUFFER: [char; 64] = ['\0'; 64];
static mut KEY_HEAD: usize = 0;
static mut KEY_TAIL: usize = 0;

fn push_key(c: char) {
    unsafe {
        let next = (KEY_HEAD + 1) % 64;
        if next != KEY_TAIL {
            KEY_BUFFER[KEY_HEAD] = c;
            KEY_HEAD = next;
        }
    }
}

fn pop_key() -> Option<char> {
    unsafe {
        if KEY_TAIL == KEY_HEAD {
            None
        } else {
            let c = KEY_BUFFER[KEY_TAIL];
            KEY_TAIL = (KEY_TAIL + 1) % 64;
            Some(c)
        }
    }
}

fn keycode_to_char(code: u16) -> Option<char> {
    match code {
        2 => Some('1'),
        3 => Some('2'),
        4 => Some('3'),
        5 => Some('4'),
        6 => Some('5'),
        7 => Some('6'),
        8 => Some('7'),
        9 => Some('8'),
        10 => Some('9'),
        11 => Some('0'),
        12 => Some('-'),
        13 => Some('='),
        14 => Some('\x08'), // Backspace
        15 => Some('\t'),
        16 => Some('q'),
        17 => Some('w'),
        18 => Some('e'),
        19 => Some('r'),
        20 => Some('t'),
        21 => Some('y'),
        22 => Some('u'),
        23 => Some('i'),
        24 => Some('o'),
        25 => Some('p'),
        26 => Some('['),
        27 => Some(']'),
        28 => Some('\n'), // Enter
        30 => Some('a'),
        31 => Some('s'),
        32 => Some('d'),
        33 => Some('f'),
        34 => Some('g'),
        35 => Some('h'),
        36 => Some('j'),
        37 => Some('k'),
        38 => Some('l'),
        39 => Some(';'),
        40 => Some('\''),
        41 => Some('`'),
        44 => Some('z'),
        45 => Some('x'),
        46 => Some('c'),
        47 => Some('v'),
        48 => Some('b'),
        49 => Some('n'),
        50 => Some('m'),
        51 => Some(','),
        52 => Some('.'),
        53 => Some('/'),
        57 => Some(' '), // Space
        _ => None,
    }
}

fn input_driver_server() {
    user_print("[Input Server] Iniciando inicialización en U-Mode...\n");
    drivers::input::init();
    user_print("[Input Server] Inicialización completada con éxito.\n");

    // Registrar el servicio "input" en el Nameserver (Tarea 3)
    user_print("[Input Server] Registrando servicio 'input' en el Nameserver (Tarea 3)...\n");
    let mut reg_msg = crate::task::IpcMessage {
        sender: 0,
        msg_type: NS_CMD_REGISTER,
        length: 16,
        reserved: 0,
        payload: [0; 32],
    };
    reg_msg.payload[0..16].copy_from_slice(&str_to_u8_16("input"));
    
    let mut res = user_ipc_send(3, &reg_msg);
    if res == 0 {
        let mut reply = crate::task::IpcMessage {
            sender: 0,
            msg_type: 0,
            length: 0,
            reserved: 0,
            payload: [0; 32],
        };
        res = user_ipc_recv(3, &mut reply);
        if res == 0 && reply.msg_type == NS_RESP_SUCCESS {
            user_print("[Input Server] Registro exitoso en el Nameserver!\n");
        } else {
            user_print("[Input Server] Error en el registro en el Nameserver.\n");
        }
    } else {
        user_print("[Input Server] Error al conectar con el Nameserver.\n");
    }

    user_print("[Input Server] Entrando en bucle de servicio IPC...\n");

    loop {
        let mut msg = crate::task::IpcMessage {
            sender: 0,
            msg_type: 0,
            length: 0,
            reserved: 0,
            payload: [0; 32],
        };
        let res = user_ipc_recv(crate::task::IPC_WILDCARD, &mut msg);
        if res == 0 {
            if msg.sender == crate::task::IPC_SENDER_NOTIFICATION {
                // Notificación de interrupción física del kernel
                // Procesar eventos de la Virtqueue
                drivers::input::process_events(|event| {
                    // event.event_type == 1 es EV_KEY, event.value == 1 es presionado
                    if event.event_type == 1 && (event.value == 1 || event.value == 2) {
                        if let Some(c) = keycode_to_char(event.code) {
                            user_print("[Input Server] Tecla presionada detectada: ");
                            let mut single_char_buf = [0u8; 4];
                            if let Some(s) = c.encode_utf8(&mut single_char_buf).get(..) {
                                user_print(s);
                            }
                            user_print("\n");
                            push_key(c);
                        }
                    }
                });
            } else {
                // Solicitud de algún cliente
                match msg.msg_type {
                    INPUT_CMD_GET_KEY => {
                        let mut reply = crate::task::IpcMessage {
                            sender: 4,
                            msg_type: 0,
                            length: 0,
                            reserved: 0,
                            payload: [0; 32],
                        };
                        if let Some(c) = pop_key() {
                            reply.msg_type = INPUT_RESP_KEY;
                            reply.payload[0] = c as u8;
                            reply.length = 1;
                        } else {
                            reply.msg_type = INPUT_RESP_EMPTY;
                        }
                        user_ipc_send(msg.sender as usize, &reply);
                    }
                    _ => {
                        user_print("[Input Server] Comando desconocido\n");
                        let reply = crate::task::IpcMessage {
                            sender: 4,
                            msg_type: NS_RESP_ERROR,
                            length: 0,
                            reserved: 0,
                            payload: [0; 32],
                        };
                        user_ipc_send(msg.sender as usize, &reply);
                    }
                }
            }
        }
    }
}

fn input_client() {
    user_print("[Input Client] Buscando el servicio 'input' en el Nameserver (Tarea 3)...\n");
    let mut lookup_msg = crate::task::IpcMessage {
        sender: 0,
        msg_type: NS_CMD_LOOKUP,
        length: 16,
        reserved: 0,
        payload: [0; 32],
    };
    lookup_msg.payload[0..16].copy_from_slice(&str_to_u8_16("input"));

    let mut input_task_id = 0;
    let mut res = user_ipc_send(3, &lookup_msg);
    if res == 0 {
        let mut reply = crate::task::IpcMessage {
            sender: 0,
            msg_type: 0,
            length: 0,
            reserved: 0,
            payload: [0; 32],
        };
        res = user_ipc_recv(3, &mut reply);
        if res == 0 && reply.msg_type == NS_RESP_SUCCESS {
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&reply.payload[16..20]);
            input_task_id = u32::from_ne_bytes(bytes) as usize;
            user_print("[Input Client] Servicio 'input' resuelto con éxito.\n");
        } else {
            user_print("[Input Client] Error al resolver el servicio 'input'.\n");
            unsafe {
                core::arch::asm!("ecall", in("a7") 2, clobber_abi("C"));
            }
        }
    } else {
        user_print("[Input Client] Error al conectar con el Nameserver.\n");
        unsafe {
            core::arch::asm!("ecall", in("a7") 2, clobber_abi("C"));
        }
    }

    user_print("[Input Client] Entrando en bucle de consulta de teclado...\n");

    loop {
        // Enviar consulta GET_KEY
        let msg = crate::task::IpcMessage {
            sender: 0,
            msg_type: INPUT_CMD_GET_KEY,
            length: 0,
            reserved: 0,
            payload: [0; 32],
        };
        res = user_ipc_send(input_task_id, &msg);
        if res == 0 {
            let mut reply = crate::task::IpcMessage {
                sender: 0,
                msg_type: 0,
                length: 0,
                reserved: 0,
                payload: [0; 32],
            };
            res = user_ipc_recv(input_task_id, &mut reply);
            if res == 0 && reply.msg_type == INPUT_RESP_KEY {
                let c = reply.payload[0] as char;
                user_print("[Input Client] Carácter leído desde el driver de teclado: ");
                let mut single_char_buf = [0u8; 4];
                if let Some(s) = c.encode_utf8(&mut single_char_buf).get(..) {
                    user_print(s);
                }
                user_print("\n");
            }
        }

        // Esperar un poco para no saturar la CPU
        for _ in 0..1000000 {
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
