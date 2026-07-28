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
    let user_entry_va2 = (window_manager_task as *const () as usize) - 0x40000000;
    let user_entry_va3 = (nameserver_task as *const () as usize) - 0x40000000;
    let user_entry_va4 = (input_driver_server as *const () as usize) - 0x40000000;
    let user_entry_va5 = (window_client_1 as *const () as usize) - 0x40000000;
    let user_entry_va6 = (window_client_2 as *const () as usize) - 0x40000000;
    let user_entry_va7 = (vfs_server as *const () as usize) - 0x40000000;
    let user_entry_va8 = (vfs_client as *const () as usize) - 0x40000000;
    let user_entry_va9 = (shell_task as *const () as usize) - 0x40000000; // <- Tarea 9: Shell
                                                                          //
    // Crear y registrar tareas secundarias (Tareas en Modo Usuario)
    task::create_user_task(1, user_entry_va1); // Tarea 1: Servidor GPU
    task::create_user_task(2, user_entry_va2); // Tarea 2: Window Manager
    task::create_user_task(3, user_entry_va3); // Tarea 3: Nameserver
    task::create_user_task(4, user_entry_va4); // Tarea 4: Servidor Input
    task::create_user_task(5, user_entry_va5); // Tarea 5: Cliente Window 1
    task::create_user_task(6, user_entry_va6); // Tarea 6: Cliente Window 2
    task::create_user_task(7, user_entry_va7); // Tarea 7: VFS Server
    task::create_user_task(8, user_entry_va8); // Tarea 8: VFS Client
    task::create_user_task(9, user_entry_va9); // Tarea 9: Shell
    sbi::print_str("[Kernel] Tareas de usuario 1 a 9 creadas.\n");

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
        // Limpiar la interrupción de temporizador pendiente programando el siguiente tick antes de wfi
        sbi::sbi_set_timer(sbi::get_time() + crate::trap::TIMER_INTERVAL);
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
            in("a1") msg as *const _,
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
            in("a1") msg as *mut _,
            clobber_abi("C"),
        );
    }
    res
}

#[inline(never)]
fn user_yield() {
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 10, // sys_yield
            clobber_abi("C"),
        );
    }
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

// Window Manager Commands
pub const WM_CMD_CREATE_WINDOW: u32 = 3001;
pub const WM_CMD_DRAW_RECT: u32     = 3002;
pub const WM_CMD_DRAW_TEXT: u32     = 3003;
pub const WM_CMD_UPDATE: u32        = 3004;

pub const WM_RESP_SUCCESS: u32      = 2000;
pub const WM_RESP_ERROR: u32        = 4000;

#[derive(Copy, Clone, Debug)]
struct Window {
    id: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    title: [u8; 16],
    bg_color: u32,
    active: bool,
}

static mut WINDOWS: [Option<Window>; 4] = [None; 4];

static mut CONSOLE_BUFFER: [u8; 128] = [0; 128];
static mut CONSOLE_LEN: usize = 0;

static mut SHELL_OUTPUT: [u8; 64] = [0; 64];
static mut SHELL_OUTPUT_LEN: usize = 0;

fn wm_composite(gpu_task_id: usize) {
    // 1. Draw desktop background (elegant dark background)
    drivers::gpu::draw_rect(0, 0, 640, 480, 0xFF1A1B26);
    
    // Draw top bar
    drivers::gpu::draw_rect(0, 0, 640, 24, 0xFF16161E);
    drivers::gpu::draw_string(10, 8, "AetherV OS  |  U-Mode Window Manager  |  Tasks: 7  |  Idle CPU: 0%", 0xFFC0CAF5);
    
    // 2. Draw windows
    unsafe {
        for idx in 0..4 {
            if let Some(ref w) = WINDOWS[idx] {
                // Border/Shadow
                drivers::gpu::draw_rect(w.x + 3, w.y + 3, w.w, w.h, 0xFF0D0E15);
                drivers::gpu::draw_rect(w.x, w.y, w.w, w.h, if w.active { 0xFF7AA2F7 } else { 0xFF565F89 });
                // Title bar
                drivers::gpu::draw_rect(w.x + 2, w.y + 2, w.w - 4, 18, if w.active { 0xFF3D59A1 } else { 0xFF24283B });
                // Client area background
                drivers::gpu::draw_rect(w.x + 2, w.y + 20, w.w - 4, w.h - 22, w.bg_color);
                
                // Title text
                let mut len = 0;
                while len < 16 && w.title[len] != 0 {
                    len += 1;
                }
                if let Ok(title_str) = core::str::from_utf8(&w.title[0..len]) {
                    drivers::gpu::draw_string(w.x + 8, w.y + 6, title_str, 0xFFFFFFFF);
                }
                // Close button [X]
                drivers::gpu::draw_string(w.x + w.w - 20, w.y + 6, "x", 0xFFF7768E);
                
                // Draw custom content if internal window
                if w.id == 0 {
                    // Draw System Monitor contents
                    let mut y_offset = w.y + 26;
                    drivers::gpu::draw_string(w.x + 8, y_offset, "PID  TASK NAME         STATUS", 0xFF9ECE6A);
                    y_offset += 12;
                    drivers::gpu::draw_string(w.x + 8, y_offset, "-----------------------------", 0xFF565F89);
                    y_offset += 12;
                    for i in 0..crate::task::MAX_TASKS {
                        let t = &crate::task::SCHEDULER.tasks[i];
                        if t.status != crate::task::TaskStatus::Unused {
                            let name = match i {
                                0 => "Idle / S-Mode   ",
                                1 => "gpu_driver_srv  ",
                                2 => "window_manager  ",
                                3 => "nameserver      ",
                                4 => "input_driver_srv",
                                5 => "window_client_1 ",
                                6 => "window_client_2 ",
                                _ => "unknown_task    ",
                            };
                            let status_str = match t.status {
                                crate::task::TaskStatus::Ready => "Ready      ",
                                crate::task::TaskStatus::Running => "Running    ",
                                crate::task::TaskStatus::BlockedSend => "BlockedSend",
                                crate::task::TaskStatus::BlockedRecv => "BlockedRecv",
                                crate::task::TaskStatus::Exited => "Exited     ",
                                _ => "Unused     ",
                            };
                            
                            let mut line_buf = [b' '; 32];
                            line_buf[0] = b'0' + (i as u8);
                            for j in 0..16 {
                                line_buf[4 + j] = name.as_bytes()[j];
                            }
                            for j in 0..11 {
                                line_buf[21 + j] = status_str.as_bytes()[j];
                            }
                            if let Ok(s) = core::str::from_utf8(&line_buf[0..32]) {
                                drivers::gpu::draw_string(w.x + 8, y_offset, s, 0xFFC0CAF5);
                            }
                            y_offset += 12;
                        }
                    }
                } else if w.id == 1 {
                    // Draw Keyboard Console contents
                    let mut y_offset = w.y + 26;
                    drivers::gpu::draw_string(w.x + 8, y_offset, "Interactive OS Shell (U-Mode)", 0xFF2AC3DE);
                    y_offset += 14;
                    drivers::gpu::draw_string(w.x + 8, y_offset, "Type command (e.g. help):", 0xFF565F89);
                    y_offset += 14;
                    
                    let mut line_buf = [b' '; 40];
                    line_buf[0] = b'>';
                    line_buf[1] = b' ';
                    let len = CONSOLE_LEN;
                    for j in 0..len {
                        line_buf[2 + j] = CONSOLE_BUFFER[j];
                    }
                    line_buf[2 + len] = b'_';
                    if let Ok(s) = core::str::from_utf8(&line_buf[0..(3 + len)]) {
                        drivers::gpu::draw_string(w.x + 8, y_offset, s, 0xFFFFFFFF);
                    }
                    y_offset += 16;
                    
                    if SHELL_OUTPUT_LEN > 0 {
                        if let Ok(out_str) = core::str::from_utf8(&SHELL_OUTPUT[0..SHELL_OUTPUT_LEN]) {
                            drivers::gpu::draw_string(w.x + 8, y_offset, out_str, 0xFFE0AF68);
                        }
                    }
                }
            }
        }
    }
    
    // 3. Flush the screen
    let flush_msg = crate::task::IpcMessage {
        sender: 0,
        msg_type: 3, // GPU_CMD_FLUSH
        length: 0,
        reserved: 0,
        payload: [0; 32],
    };
    user_ipc_send(gpu_task_id, &flush_msg);
    
    let mut reply = crate::task::IpcMessage {
        sender: 0,
        msg_type: 0,
        length: 0,
        reserved: 0,
        payload: [0; 32],
    };
    user_ipc_recv(gpu_task_id, &mut reply);
}

fn window_manager_task() {
    let mut res;
    user_print("[WM] Buscando el servicio 'display' en el Nameserver (Tarea 3)...\n");
    let mut lookup_msg = crate::task::IpcMessage {
        sender: 0,
        msg_type: NS_CMD_LOOKUP,
        length: 16,
        reserved: 0,
        payload: [0; 32],
    };
    lookup_msg.payload[0..16].copy_from_slice(&str_to_u8_16("display"));

    let gpu_task_id;
    loop {
        let res = user_ipc_send(3, &lookup_msg);
        if res == 0 {
            let mut reply = crate::task::IpcMessage {
                sender: 0,
                msg_type: 0,
                length: 0,
                reserved: 0,
                payload: [0; 32],
            };
            let res2 = user_ipc_recv(3, &mut reply);
            if res2 == 0 && reply.msg_type == NS_RESP_SUCCESS {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(&reply.payload[16..20]);
                gpu_task_id = u32::from_ne_bytes(bytes) as usize;
                user_print("[WM] Servicio 'display' resuelto con éxito.\n");
                break;
            }
        }
        user_yield();
    }

    // Registrar el servicio "wm" en el Nameserver (Tarea 3)
    user_print("[WM] Registrando servicio 'wm' en el Nameserver (Tarea 3)...\n");
    let mut reg_msg = crate::task::IpcMessage {
        sender: 0,
        msg_type: NS_CMD_REGISTER,
        length: 16,
        reserved: 0,
        payload: [0; 32],
    };
    reg_msg.payload[0..16].copy_from_slice(&str_to_u8_16("wm"));
    
    res = user_ipc_send(3, &reg_msg);
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
            user_print("[WM] Registro de 'wm' exitoso en el Nameserver!\n");
        } else {
            user_print("[WM] Error en el registro de 'wm'.\n");
        }
    } else {
        user_print("[WM] Error al conectar con el Nameserver.\n");
    }

    // Inicializar ventanas locales
    unsafe {
        let mut title0 = [0u8; 16];
        title0[0..14].copy_from_slice(b"System Monitor");
        WINDOWS[0] = Some(Window {
            id: 0,
            x: 30,
            y: 40,
            w: 270,
            h: 180,
            title: title0,
            bg_color: 0xFF1F2335,
            active: true,
        });

        let mut title1 = [0u8; 16];
        title1[0..16].copy_from_slice(b"Keyboard Console");
        WINDOWS[1] = Some(Window {
            id: 1,
            x: 330,
            y: 40,
            w: 280,
            h: 180,
            title: title1,
            bg_color: 0xFF1A1B26,
            active: false,
        });
    }

    // Dibujo inicial y refresco
    wm_composite(gpu_task_id);

    user_print("[WM] Entrando en bucle de servicio de ventanas...\n");

    loop {
        let mut msg = crate::task::IpcMessage {
            sender: 0,
            msg_type: 0,
            length: 0,
            reserved: 0,
            payload: [0; 32],
        };
        // Espera de comandos de dibujo o de tecla
        let res = user_ipc_recv(crate::task::IPC_WILDCARD, &mut msg);
        if res == 0 {
            match msg.msg_type {
                2004 => {
                    // Tecla presionada
                    let c = msg.payload[0] as char;
                    unsafe {
                        if let Some(ref mut win0) = WINDOWS[0] { win0.active = false; }
                        if let Some(ref mut win1) = WINDOWS[1] { win1.active = true; }
                        
                        if c == '\x08' {
                            if CONSOLE_LEN > 0 {
                                CONSOLE_LEN -= 1;
                            }
                        } else if c == '\n' || c == '\r' {
                            let mut cmd = [0u8; 32];
                            let len = core::cmp::min(CONSOLE_LEN, 32);
                            for i in 0..len {
                                cmd[i] = CONSOLE_BUFFER[i];
                            }
                            
                            SHELL_OUTPUT_LEN = 0;
                            if len >= 4 && &cmd[0..4] == b"help" {
                                let resp = b"Cmds: help, clear, logo, color";
                                SHELL_OUTPUT[0..resp.len()].copy_from_slice(resp);
                                SHELL_OUTPUT_LEN = resp.len();
                            } else if len >= 5 && &cmd[0..5] == b"clear" {
                                SHELL_OUTPUT_LEN = 0;
                            } else if len >= 4 && &cmd[0..4] == b"logo" {
                                let resp = b"AetherV OS - Minimalist RISC-V";
                                SHELL_OUTPUT[0..resp.len()].copy_from_slice(resp);
                                SHELL_OUTPUT_LEN = resp.len();
                            } else if len >= 5 && &cmd[0..5] == b"color" {
                                let resp = b"Color updated!";
                                SHELL_OUTPUT[0..resp.len()].copy_from_slice(resp);
                                SHELL_OUTPUT_LEN = resp.len();
                                if let Some(ref mut win) = WINDOWS[1] {
                                    win.bg_color = 0xFF2C1C30; // Dark plum color
                                }
                            } else {
                                let resp = b"Unknown command. Try help.";
                                SHELL_OUTPUT[0..resp.len()].copy_from_slice(resp);
                                SHELL_OUTPUT_LEN = resp.len();
                            }
                            CONSOLE_LEN = 0;
                        } else {
                            if CONSOLE_LEN < 30 {
                                CONSOLE_BUFFER[CONSOLE_LEN] = c as u8;
                                CONSOLE_LEN += 1;
                            }
                        }
                    }
                    wm_composite(gpu_task_id);
                }
                WM_CMD_CREATE_WINDOW => {
                    let mut title = [0u8; 16];
                    title.copy_from_slice(&msg.payload[0..16]);
                    let x = msg.payload[16] as usize;
                    let y = msg.payload[17] as usize;
                    let w = msg.payload[18] as usize * 2;
                    let h = msg.payload[19] as usize * 2;
                    let r = msg.payload[20] as u32;
                    let g = msg.payload[21] as u32;
                    let b = msg.payload[22] as u32;
                    let bg_color = 0xFF000000 | (r << 16) | (g << 8) | b;
                    
                    let mut found_idx = None;
                    unsafe {
                        for idx in 2..4 {
                            if WINDOWS[idx].is_none() {
                                WINDOWS[idx] = Some(Window {
                                    id: idx,
                                    x,
                                    y,
                                    w,
                                    h,
                                    title,
                                    bg_color,
                                    active: false,
                                });
                                found_idx = Some(idx);
                                break;
                            }
                        }
                    }
                    
                    let mut reply = crate::task::IpcMessage {
                        sender: 2,
                        msg_type: if found_idx.is_some() { WM_RESP_SUCCESS } else { WM_RESP_ERROR },
                        length: 4,
                        reserved: 0,
                        payload: [0; 32],
                    };
                    if let Some(idx) = found_idx {
                        let bytes = (idx as u32).to_ne_bytes();
                        reply.payload[0..4].copy_from_slice(&bytes);
                    }
                    user_ipc_send(msg.sender as usize, &reply);
                    
                    wm_composite(gpu_task_id);
                }
                WM_CMD_DRAW_RECT => {
                    let win_id = msg.payload[0] as usize;
                    let rx = msg.payload[1] as usize;
                    let ry = msg.payload[2] as usize;
                    let rw = msg.payload[3] as usize;
                    let rh = msg.payload[4] as usize;
                    let r = msg.payload[5] as u32;
                    let g = msg.payload[6] as u32;
                    let b = msg.payload[7] as u32;
                    let color = 0xFF000000 | (r << 16) | (g << 8) | b;
                    
                    unsafe {
                        if win_id < 4 {
                            if let Some(ref win) = WINDOWS[win_id] {
                                let px = win.x + 2 + rx;
                                let py = win.y + 20 + ry;
                                let max_w = win.w - 4;
                                let max_h = win.h - 22;
                                let draw_w = if rx + rw > max_w { max_w.saturating_sub(rx) } else { rw };
                                let draw_h = if ry + rh > max_h { max_h.saturating_sub(ry) } else { rh };
                                drivers::gpu::draw_rect(px, py, draw_w, draw_h, color);
                            }
                        }
                    }
                    let reply = crate::task::IpcMessage {
                        sender: 2,
                        msg_type: WM_RESP_SUCCESS,
                        length: 0,
                        reserved: 0,
                        payload: [0; 32],
                    };
                    user_ipc_send(msg.sender as usize, &reply);
                }
                WM_CMD_DRAW_TEXT => {
                    let win_id = msg.payload[0] as usize;
                    let tx = msg.payload[1] as usize;
                    let ty = msg.payload[2] as usize;
                    let r = msg.payload[3] as u32;
                    let g = msg.payload[4] as u32;
                    let b = msg.payload[5] as u32;
                    let color = 0xFF000000 | (r << 16) | (g << 8) | b;
                    
                    let mut text_buf = [0u8; 20];
                    text_buf.copy_from_slice(&msg.payload[6..26]);
                    
                    unsafe {
                        if win_id < 4 {
                            if let Some(ref win) = WINDOWS[win_id] {
                                let px = win.x + 2 + tx;
                                let py = win.y + 20 + ty;
                                let mut len = 0;
                                while len < 20 && text_buf[len] != 0 {
                                    len += 1;
                                }
                                if let Ok(s) = core::str::from_utf8(&text_buf[0..len]) {
                                    drivers::gpu::draw_string(px, py, s, color);
                                }
                            }
                        }
                    }
                    let reply = crate::task::IpcMessage {
                        sender: 2,
                        msg_type: WM_RESP_SUCCESS,
                        length: 0,
                        reserved: 0,
                        payload: [0; 32],
                    };
                    user_ipc_send(msg.sender as usize, &reply);
                }
                WM_CMD_UPDATE => {
                    let reply = crate::task::IpcMessage {
                        sender: 2,
                        msg_type: WM_RESP_SUCCESS,
                        length: 0,
                        reserved: 0,
                        payload: [0; 32],
                    };
                    user_ipc_send(msg.sender as usize, &reply);
                    
                    let flush_msg = crate::task::IpcMessage {
                        sender: 0,
                        msg_type: 3, // GPU_CMD_FLUSH
                        length: 0,
                        reserved: 0,
                        payload: [0; 32],
                    };
                    user_ipc_send(gpu_task_id, &flush_msg);
                    
                    let mut reply_gpu = crate::task::IpcMessage {
                        sender: 0,
                        msg_type: 0,
                        length: 0,
                        reserved: 0,
                        payload: [0; 32],
                    };
                    user_ipc_recv(gpu_task_id, &mut reply_gpu);
                }
                _ => {
                    let reply = crate::task::IpcMessage {
                        sender: 2,
                        msg_type: WM_RESP_ERROR,
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
                    if event.event_type == 1 && (event.value == 1 || event.value == 2) {
                        if let Some(c) = keycode_to_char(event.code) {
                            user_print("[Input Server] Tecla presionada detectada: ");
                            let mut single_char_buf = [0u8; 4];
                            if let Some(s) = c.encode_utf8(&mut single_char_buf).get(..) {
                                user_print(s);
                            }
                            user_print("\n");
                            
                            // Intentar enviar al Window Manager directly
                            let mut lookup_msg = crate::task::IpcMessage {
                                sender: 0,
                                msg_type: NS_CMD_LOOKUP,
                                length: 16,
                                reserved: 0,
                                payload: [0; 32],
                            };
                            lookup_msg.payload[0..16].copy_from_slice(&str_to_u8_16("wm"));
                            
                            let mut reply = crate::task::IpcMessage {
                                sender: 0,
                                msg_type: 0,
                                length: 0,
                                reserved: 0,
                                payload: [0; 32],
                            };
                            
                            let mut wm_task_id = 0;
                            if user_ipc_send(3, &lookup_msg) == 0 {
                                if user_ipc_recv(3, &mut reply) == 0 && reply.msg_type == NS_RESP_SUCCESS {
                                    let mut bytes = [0u8; 4];
                                    bytes.copy_from_slice(&reply.payload[16..20]);
                                    wm_task_id = u32::from_ne_bytes(bytes) as usize;
                                }
                            }
                            
                            if wm_task_id != 0 {
                                let mut wm_msg = crate::task::IpcMessage {
                                    sender: 0,
                                    msg_type: 2004, // Tecla
                                    length: 1,
                                    reserved: 0,
                                    payload: [0; 32],
                                };
                                wm_msg.payload[0] = c as u8;
                                user_ipc_send(wm_task_id, &wm_msg);
                            }
                            
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

fn u32_to_str(val: u32, buf: &mut [u8]) -> usize {
    if val == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut temp = val;
    let mut len = 0;
    while temp > 0 {
        len += 1;
        temp /= 10;
    }
    let mut temp = val;
    for i in (0..len).rev() {
        buf[i] = b'0' + (temp % 10) as u8;
        temp /= 10;
    }
    len
}

fn window_client_1() {
    user_print("[Client 1] Buscando 'wm' en el Nameserver...\n");
    let mut lookup_msg = crate::task::IpcMessage {
        sender: 0,
        msg_type: NS_CMD_LOOKUP,
        length: 16,
        reserved: 0,
        payload: [0; 32],
    };
    lookup_msg.payload[0..16].copy_from_slice(&str_to_u8_16("wm"));

    let wm_task_id;
    loop {
        let mut reply = crate::task::IpcMessage {
            sender: 0,
            msg_type: 0,
            length: 0,
            reserved: 0,
            payload: [0; 32],
        };
        let res = user_ipc_send(3, &lookup_msg);
        if res == 0 {
            let res2 = user_ipc_recv(3, &mut reply);
            if res2 == 0 && reply.msg_type == NS_RESP_SUCCESS {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(&reply.payload[16..20]);
                wm_task_id = u32::from_ne_bytes(bytes) as usize;
                break;
            }
        }
        user_yield();
    }
    user_print("[Client 1] Conectado al Window Manager!\n");

    // Crear ventana: "Bouncing Ball"
    // x = 30, y = 240, w = 135 (270), h = 95 (190)
    let mut create_msg = crate::task::IpcMessage {
        sender: 0,
        msg_type: WM_CMD_CREATE_WINDOW,
        length: 24,
        reserved: 0,
        payload: [0; 32],
    };
    create_msg.payload[0..16].copy_from_slice(b"Bouncing Ball\0\0\0");
    create_msg.payload[16] = 30;  // x
    create_msg.payload[17] = 240; // y
    create_msg.payload[18] = 135; // w / 2
    create_msg.payload[19] = 95;  // h / 2
    create_msg.payload[20] = 30;  // r
    create_msg.payload[21] = 30;  // g
    create_msg.payload[22] = 46;  // b

    let mut reply = crate::task::IpcMessage {
        sender: 0,
        msg_type: 0,
        length: 0,
        reserved: 0,
        payload: [0; 32],
    };
    let mut win_id = 0;
    if user_ipc_send(wm_task_id, &create_msg) == 0 {
        if user_ipc_recv(wm_task_id, &mut reply) == 0 && reply.msg_type == WM_RESP_SUCCESS {
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&reply.payload[0..4]);
            win_id = u32::from_ne_bytes(bytes) as usize;
        }
    }

    let mut ball_x: i32 = 20;
    let mut ball_y: i32 = 30;
    let mut ball_dx: i32 = 6;
    let mut ball_dy: i32 = 4;
    
    // Bounds: width = 266, height = 168
    let max_w: i32 = 266 - 8;
    let max_h: i32 = 168 - 8;

    loop {
        // Mover la bola
        ball_x += ball_dx;
        ball_y += ball_dy;
        if ball_x <= 4 || ball_x >= max_w {
            ball_dx = -ball_dx;
        }
        if ball_y <= 4 || ball_y >= max_h {
            ball_dy = -ball_dy;
        }

        // Limpiar el contenido de la ventana
        let mut clear_msg = crate::task::IpcMessage {
            sender: 0,
            msg_type: WM_CMD_DRAW_RECT,
            length: 8,
            reserved: 0,
            payload: [0; 32],
        };
        clear_msg.payload[0] = win_id as u8;
        clear_msg.payload[1] = 0;   // rx
        clear_msg.payload[2] = 0;   // ry
        clear_msg.payload[3] = 255; // rw
        clear_msg.payload[4] = 168; // rh
        clear_msg.payload[5] = 30;  // r
        clear_msg.payload[6] = 30;  // g
        clear_msg.payload[7] = 46;  // b
        user_ipc_send(wm_task_id, &clear_msg);
        user_ipc_recv(wm_task_id, &mut reply);

        // Dibujar texto informativo
        let mut text_msg = crate::task::IpcMessage {
            sender: 0,
            msg_type: WM_CMD_DRAW_TEXT,
            length: 26,
            reserved: 0,
            payload: [0; 32],
        };
        text_msg.payload[0] = win_id as u8;
        text_msg.payload[1] = 10; // tx
        text_msg.payload[2] = 10; // ty
        text_msg.payload[3] = 255; // r
        text_msg.payload[4] = 255; // g
        text_msg.payload[5] = 255; // b
        text_msg.payload[6..26].copy_from_slice(b"Bouncing Ball Demo.\0");
        user_ipc_send(wm_task_id, &text_msg);
        user_ipc_recv(wm_task_id, &mut reply);

        // Dibujar la bola 'O'
        let mut ball_msg = crate::task::IpcMessage {
            sender: 0,
            msg_type: WM_CMD_DRAW_TEXT,
            length: 26,
            reserved: 0,
            payload: [0; 32],
        };
        ball_msg.payload[0] = win_id as u8;
        ball_msg.payload[1] = ball_x as u8;
        ball_msg.payload[2] = ball_y as u8;
        ball_msg.payload[3] = 248; // r
        ball_msg.payload[4] = 196; // g
        ball_msg.payload[5] = 113; // b
        ball_msg.payload[6..26].copy_from_slice(b"O\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0");
        user_ipc_send(wm_task_id, &ball_msg);
        user_ipc_recv(wm_task_id, &mut reply);

        // Actualizar la pantalla
        let update_msg = crate::task::IpcMessage {
            sender: 0,
            msg_type: WM_CMD_UPDATE,
            length: 0,
            reserved: 0,
            payload: [0; 32],
        };
        user_ipc_send(wm_task_id, &update_msg);
        user_ipc_recv(wm_task_id, &mut reply);

        // Retardo (~30 fps)
        for _ in 0..10 {
            user_yield();
        }
    }
}

fn window_client_2() {
    user_print("[Client 2] Buscando 'wm' en el Nameserver...\n");
    let mut lookup_msg = crate::task::IpcMessage {
        sender: 0,
        msg_type: NS_CMD_LOOKUP,
        length: 16,
        reserved: 0,
        payload: [0; 32],
    };
    lookup_msg.payload[0..16].copy_from_slice(&str_to_u8_16("wm"));

    let wm_task_id;
    loop {
        let mut reply = crate::task::IpcMessage {
            sender: 0,
            msg_type: 0,
            length: 0,
            reserved: 0,
            payload: [0; 32],
        };
        let res = user_ipc_send(3, &lookup_msg);
        if res == 0 {
            let res2 = user_ipc_recv(3, &mut reply);
            if res2 == 0 && reply.msg_type == NS_RESP_SUCCESS {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(&reply.payload[16..20]);
                wm_task_id = u32::from_ne_bytes(bytes) as usize;
                break;
            }
        }
        user_yield();
    }
    user_print("[Client 2] Conectado al Window Manager!\n");

    // Crear ventana: "Performance Counter"
    // x = 330, y = 240, w = 140 (280), h = 95 (190)
    let mut create_msg = crate::task::IpcMessage {
        sender: 0,
        msg_type: WM_CMD_CREATE_WINDOW,
        length: 24,
        reserved: 0,
        payload: [0; 32],
    };
    create_msg.payload[0..16].copy_from_slice(b"Performance\0\0\0\0\0");
    create_msg.payload[16] = 165; // x (scaled to 330)
    create_msg.payload[17] = 240; // y
    create_msg.payload[18] = 140; // w / 2
    create_msg.payload[19] = 95;  // h / 2
    create_msg.payload[20] = 26;  // r
    create_msg.payload[21] = 27;  // g
    create_msg.payload[22] = 38;  // b

    let mut reply = crate::task::IpcMessage {
        sender: 0,
        msg_type: 0,
        length: 0,
        reserved: 0,
        payload: [0; 32],
    };
    let mut win_id = 0;
    if user_ipc_send(wm_task_id, &create_msg) == 0 {
        if user_ipc_recv(wm_task_id, &mut reply) == 0 && reply.msg_type == WM_RESP_SUCCESS {
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&reply.payload[0..4]);
            win_id = u32::from_ne_bytes(bytes) as usize;
        }
    }

    let mut counter: u32 = 0;
    loop {
        counter = counter.wrapping_add(1);

        // Limpiar el contenido de la ventana
        let mut clear_msg = crate::task::IpcMessage {
            sender: 0,
            msg_type: WM_CMD_DRAW_RECT,
            length: 8,
            reserved: 0,
            payload: [0; 32],
        };
        clear_msg.payload[0] = win_id as u8;
        clear_msg.payload[1] = 0;   // rx
        clear_msg.payload[2] = 0;   // ry
        clear_msg.payload[3] = 255; // rw
        clear_msg.payload[4] = 168; // rh
        clear_msg.payload[5] = 26;  // r
        clear_msg.payload[6] = 27;  // g
        clear_msg.payload[7] = 38;  // b
        user_ipc_send(wm_task_id, &clear_msg);
        user_ipc_recv(wm_task_id, &mut reply);

        // Dibujar texto del contador
        let mut text_msg = crate::task::IpcMessage {
            sender: 0,
            msg_type: WM_CMD_DRAW_TEXT,
            length: 26,
            reserved: 0,
            payload: [0; 32],
        };
        text_msg.payload[0] = win_id as u8;
        text_msg.payload[1] = 15; // tx
        text_msg.payload[2] = 20; // ty
        text_msg.payload[3] = 122; // r
        text_msg.payload[4] = 162; // g
        text_msg.payload[5] = 247; // b
        
        let mut text_buf = [0u8; 20];
        text_buf[0..9].copy_from_slice(b"Counter: ");
        let mut num_buf = [0u8; 10];
        let num_len = u32_to_str(counter, &mut num_buf);
        for i in 0..num_len {
            text_buf[9 + i] = num_buf[i];
        }
        text_msg.payload[6..26].copy_from_slice(&text_buf);
        user_ipc_send(wm_task_id, &text_msg);
        user_ipc_recv(wm_task_id, &mut reply);

        // Dibujar barra de progreso
        let progress = (counter / 5) % 15;
        let mut prog_buf = [b' '; 20];
        prog_buf[0] = b'[';
        for i in 0..15 {
            if (i as u32) < progress {
                prog_buf[1 + i] = b'=';
            } else if (i as u32) == progress {
                prog_buf[1 + i] = b'>';
            } else {
                prog_buf[1 + i] = b' ';
            }
        }
        prog_buf[16] = b']';
        
        let mut prog_msg = crate::task::IpcMessage {
            sender: 0,
            msg_type: WM_CMD_DRAW_TEXT,
            length: 26,
            reserved: 0,
            payload: [0; 32],
        };
        prog_msg.payload[0] = win_id as u8;
        prog_msg.payload[1] = 15; // tx
        prog_msg.payload[2] = 45; // ty
        prog_msg.payload[3] = 187; // r
        prog_msg.payload[4] = 154; // g
        prog_msg.payload[5] = 247; // b
        prog_msg.payload[6..26].copy_from_slice(&prog_buf);
        user_ipc_send(wm_task_id, &prog_msg);
        user_ipc_recv(wm_task_id, &mut reply);

        // Dibujar texto adicional
        let mut title_msg = crate::task::IpcMessage {
            sender: 0,
            msg_type: WM_CMD_DRAW_TEXT,
            length: 26,
            reserved: 0,
            payload: [0; 32],
        };
        title_msg.payload[0] = win_id as u8;
        title_msg.payload[1] = 15; // tx
        title_msg.payload[2] = 80; // ty
        title_msg.payload[3] = 94; // r
        title_msg.payload[4] = 211; // g
        title_msg.payload[5] = 162; // b
        title_msg.payload[6..26].copy_from_slice(b"System tick active\0\0");
        user_ipc_send(wm_task_id, &title_msg);
        user_ipc_recv(wm_task_id, &mut reply);

        // Actualizar la pantalla
        let update_msg = crate::task::IpcMessage {
            sender: 0,
            msg_type: WM_CMD_UPDATE,
            length: 0,
            reserved: 0,
            payload: [0; 32],
        };
        user_ipc_send(wm_task_id, &update_msg);
        user_ipc_recv(wm_task_id, &mut reply);

        // Retardo
        for _ in 0..10 {
            user_yield();
        }
    }
}

struct RamFile {
    path: &'static [u8],
    content: &'static [u8],
}

static RAM_DISK: [RamFile; 2] = [
    RamFile {
        path: b"/readme.txt",
        content: b"AetherV OS - Microkernel RISC-V\n", // 32 bytes exactos
    },
    RamFile {
        path: b"/config.sys",
        content: b"version=1.2-alpha\n",
    },
];

fn vfs_server() {
    user_print("[VFS Server] Iniciando en U-Mode...\n");

    // 1. Registrar servicio "vfs" en Nameserver (Tarea 3) con retry loop
    let mut reg_msg = crate::task::IpcMessage {
        sender: 0,
        msg_type: NS_CMD_REGISTER,
        length: 16,
        reserved: 0,
        payload: [0; 32],
    };
    reg_msg.payload[0..16].copy_from_slice(&str_to_u8_16("vfs"));

    loop {
        let res = user_ipc_send(3, &reg_msg);
        if res == 0 {
            let mut reply = crate::task::IpcMessage {
                sender: 0,
                msg_type: 0,
                length: 0,
                reserved: 0,
                payload: [0; 32],
            };
            let res2 = user_ipc_recv(3, &mut reply);
            if res2 == 0 && reply.msg_type == NS_RESP_SUCCESS {
                user_print("[VFS Server] Registro 'vfs' exitoso en Nameserver!\n");
                break;
            }
        }
        user_yield();
    }

    user_print("[VFS Server] Entrando en bucle de servicio IPC...\n");

    // Tabla simple de estado por cliente para recordar el archivo abierto
    let mut open_file_idx: [Option<usize>; crate::task::MAX_TASKS] = [None; crate::task::MAX_TASKS];

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
            let client_id = msg.sender as usize;
            match msg.msg_type {
                VFS_CMD_OPEN => {
                    let mut path_len = 0;
                    while path_len < 32 && msg.payload[path_len] != 0 {
                        path_len += 1;
                    }

                    let requested_path = &msg.payload[..path_len];
                    let mut found = None;

                    for (idx, file) in RAM_DISK.iter().enumerate() {
                        if file.path == requested_path {
                            found = Some(idx);
                            break;
                        }
                    }

                    let mut reply = crate::task::IpcMessage {
                        sender: 7,
                        msg_type: VFS_RESP_ERR,
                        length: 0,
                        reserved: 0,
                        payload: [0; 32],
                    };

                    if let Some(file_idx) = found {
                        open_file_idx[client_id] = Some(file_idx);
                        reply.msg_type = VFS_RESP_OK;
                        let size_bytes = (RAM_DISK[file_idx].content.len() as u32).to_ne_bytes();
                        reply.payload[0..4].copy_from_slice(&size_bytes);
                        reply.length = 4;
                        user_print("[VFS Server] Archivo encontrado y abierto.\n");
                    } else {
                        user_print("[VFS Server] Archivo no encontrado.\n");
                    }

                    user_ipc_send(client_id, &reply);
                }
                VFS_CMD_READ => {
                    let mut reply = crate::task::IpcMessage {
                        sender: 7,
                        msg_type: VFS_RESP_ERR,
                        length: 0,
                        reserved: 0,
                        payload: [0; 32],
                    };

                    if let Some(file_idx) = open_file_idx[client_id] {
                        let data = RAM_DISK[file_idx].content;
                        reply = make_vfs_read_resp(data);
                        reply.sender = 7;
                    }

                    user_ipc_send(client_id, &reply);
                }
                VFS_CMD_CLOSE => {
                    open_file_idx[client_id] = None;
                    let reply = crate::task::IpcMessage {
                        sender: 7,
                        msg_type: VFS_RESP_OK,
                        length: 0,
                        reserved: 0,
                        payload: [0; 32],
                    };
                    user_ipc_send(client_id, &reply);
                }
                _ => {
                    let reply = crate::task::IpcMessage {
                        sender: 7,
                        msg_type: VFS_RESP_ERR,
                        length: 0,
                        reserved: 0,
                        payload: [0; 32],
                    };
                    user_ipc_send(client_id, &reply);
                }
            }
        }
    }
}

fn vfs_client() {
    user_print("[VFS Client] Buscando 'vfs' en el Nameserver...\n");
    let mut lookup_msg = crate::task::IpcMessage {
        sender: 0,
        msg_type: NS_CMD_LOOKUP,
        length: 16,
        reserved: 0,
        payload: [0; 32],
    };
    lookup_msg.payload[0..16].copy_from_slice(&str_to_u8_16("vfs"));

    let vfs_task_id;
    loop {
        let mut reply = crate::task::IpcMessage {
            sender: 0,
            msg_type: 0,
            length: 0,
            reserved: 0,
            payload: [0; 32],
        };
        let res = user_ipc_send(3, &lookup_msg);
        if res == 0 {
            let res2 = user_ipc_recv(3, &mut reply);
            if res2 == 0 && reply.msg_type == NS_RESP_SUCCESS {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(&reply.payload[16..20]);
                vfs_task_id = u32::from_ne_bytes(bytes) as usize;
                break;
            }
        }
        user_yield();
    }
    user_print("[VFS Client] Conectado con éxito al VFS Server!\n");

    // 1. Abrir el archivo "/readme.txt"
    user_print("[VFS Client] Solicitando abrir '/readme.txt'...\n");
    let open_msg = make_vfs_open_msg(b"/readme.txt");
    let mut reply = crate::task::IpcMessage {
        sender: 0,
        msg_type: 0,
        length: 0,
        reserved: 0,
        payload: [0; 32],
    };

    if user_ipc_send(vfs_task_id, &open_msg) == 0 {
        if user_ipc_recv(vfs_task_id, &mut reply) == 0 && reply.msg_type == VFS_RESP_OK {
            user_print("[VFS Client] Archivo '/readme.txt' abierto con éxito.\n");

            // 2. Leer el contenido por IPC
            let read_msg = crate::task::IpcMessage {
                sender: 0,
                msg_type: VFS_CMD_READ,
                length: 0,
                reserved: 0,
                payload: [0; 32],
            };

            if user_ipc_send(vfs_task_id, &read_msg) == 0 {
                if user_ipc_recv(vfs_task_id, &mut reply) == 0 && reply.msg_type == VFS_RESP_OK {
                    user_print("[VFS Client] Contenido del archivo leido vía IPC:\n>>> ");
                    let len = reply.length as usize;
                    if let Ok(content_str) = core::str::from_utf8(&reply.payload[..len]) {
                        user_print(content_str);
                    }
                    user_print("<<<\n");
                }
            }

            // 3. Cerrar archivo
            let close_msg = crate::task::IpcMessage {
                sender: 0,
                msg_type: VFS_CMD_CLOSE,
                length: 0,
                reserved: 0,
                payload: [0; 32],
            };
            user_ipc_send(vfs_task_id, &close_msg);
            user_ipc_recv(vfs_task_id, &mut reply);
        }
    }

    loop {
        user_yield();
    }
}

// ==========================================
// COMANDOS Y MENSAJES IPC PARA EL VFS SERVER
// ==========================================

pub const VFS_CMD_OPEN: u32 = 100;
pub const VFS_CMD_READ: u32 = 101;
pub const VFS_CMD_CLOSE: u32 = 102;

pub const VFS_RESP_OK: u32 = 200;
pub const VFS_RESP_ERR: u32 = 400;

/// Helper para crear una petición de apertura de archivo en U-Mode
pub fn make_vfs_open_msg(path: &[u8]) -> task::IpcMessage {
    let mut msg = task::IpcMessage {
        sender: 0,
        msg_type: VFS_CMD_OPEN,
        length: path.len() as u32,
        reserved: 0,
        payload: [0; 32],
    };
    let copy_len = path.len().min(32);
    msg.payload[..copy_len].copy_from_slice(&path[..copy_len]);
    msg
}

/// Helper para responder desde el VFS Server con datos del archivo
pub fn make_vfs_read_resp(data: &[u8]) -> task::IpcMessage {
    let copy_len = data.len().min(32);
    let mut msg = task::IpcMessage {
        sender: 0,
        msg_type: VFS_RESP_OK,
        length: copy_len as u32,
        reserved: 0,
        payload: [0; 32],
    };
    msg.payload[..copy_len].copy_from_slice(&data[..copy_len]);
    msg
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

// ==========================================
// FASE 7: SHELL INTERACTIVA EN MODO USUARIO
// ==========================================

pub const VFS_CMD_LIST: u32 = 103;

fn shell_task() {
    user_print("[Shell] Iniciando consola interactiva en U-Mode...\n");

    // 1. Resolver direcciones de servicios en el Nameserver (Tarea 3)
    let vfs_task_id = loop {
        let mut lookup_msg = crate::task::IpcMessage {
            sender: 0,
            msg_type: NS_CMD_LOOKUP,
            length: 16,
            reserved: 0,
            payload: [0; 32],
        };
        lookup_msg.payload[0..16].copy_from_slice(&str_to_u8_16("vfs"));
        if user_ipc_send(3, &lookup_msg) == 0 {
            let mut reply = crate::task::IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
            if user_ipc_recv(3, &mut reply) == 0 && reply.msg_type == NS_RESP_SUCCESS {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(&reply.payload[16..20]);
                break u32::from_ne_bytes(bytes) as usize;
            }
        }
        user_yield();
    };

    let input_task_id = loop {
        let mut lookup_msg = crate::task::IpcMessage {
            sender: 0,
            msg_type: NS_CMD_LOOKUP,
            length: 16,
            reserved: 0,
            payload: [0; 32],
        };
        lookup_msg.payload[0..16].copy_from_slice(&str_to_u8_16("input"));
        if user_ipc_send(3, &lookup_msg) == 0 {
            let mut reply = crate::task::IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
            if user_ipc_recv(3, &mut reply) == 0 && reply.msg_type == NS_RESP_SUCCESS {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(&reply.payload[16..20]);
                break u32::from_ne_bytes(bytes) as usize;
            }
        }
        user_yield();
    };

    let wm_task_id = loop {
        let mut lookup_msg = crate::task::IpcMessage {
            sender: 0,
            msg_type: NS_CMD_LOOKUP,
            length: 16,
            reserved: 0,
            payload: [0; 32],
        };
        lookup_msg.payload[0..16].copy_from_slice(&str_to_u8_16("wm"));
        if user_ipc_send(3, &lookup_msg) == 0 {
            let mut reply = crate::task::IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
            if user_ipc_recv(3, &mut reply) == 0 && reply.msg_type == NS_RESP_SUCCESS {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(&reply.payload[16..20]);
                break u32::from_ne_bytes(bytes) as usize;
            }
        }
        user_yield();
    };

    user_print("[Shell] Servicios 'vfs', 'input' y 'wm' vinculados.\n");
    user_print("aetherv-shell> ");

    let mut cmd_buf = [0u8; 32];
    let mut cmd_len = 0;

    // Helper interno para renderizado dual (Consola Serial + WM)
    let shell_out = |s: &str, wm_id: usize| {
        user_print(s);
        let mut msg = crate::task::IpcMessage {
            sender: 0,
            msg_type: WM_CMD_DRAW_TEXT,
            length: 26,
            reserved: 0,
            payload: [0; 32],
        };
        msg.payload[0] = 1; // Window ID 1 (Keyboard Console)
        msg.payload[1] = 8;
        msg.payload[2] = 80;
        msg.payload[3] = 255; msg.payload[4] = 255; msg.payload[5] = 255;
        let bytes = s.as_bytes();
        let copy_len = bytes.len().min(20);
        msg.payload[6..6 + copy_len].copy_from_slice(&bytes[..copy_len]);
        
        let mut reply = crate::task::IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
        user_ipc_send(wm_id, &msg);
        user_ipc_recv(wm_id, &mut reply);
    };

    loop {
        // Polling de teclado vía IPC al Input Driver
        let req = crate::task::IpcMessage {
            sender: 0,
            msg_type: INPUT_CMD_GET_KEY,
            length: 0,
            reserved: 0,
            payload: [0; 32],
        };
        
        if user_ipc_send(input_task_id, &req) == 0 {
            let mut reply = crate::task::IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
            if user_ipc_recv(input_task_id, &mut reply) == 0 && reply.msg_type == INPUT_RESP_KEY {
                let c = reply.payload[0] as char;
                
                if c == '\n' || c == '\r' {
                    user_print("\n");
                    if cmd_len > 0 {
                        let cmd_str = core::str::from_utf8(&cmd_buf[..cmd_len]).unwrap_or("");
                        
                        if cmd_str == "help" {
                            shell_out("Cmds: help, ls, cat <file>, clear, info", wm_task_id);
                            user_print("\n");
                        } else if cmd_str == "ls" {
                            shell_out("/readme.txt  /config.sys", wm_task_id);
                            user_print("\n");
                        } else if cmd_str.starts_with("cat ") {
                            let path = &cmd_str[4..];
                            let open_msg = make_vfs_open_msg(path.as_bytes());
                            let mut vfs_reply = crate::task::IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
                            
                            if user_ipc_send(vfs_task_id, &open_msg) == 0 && user_ipc_recv(vfs_task_id, &mut vfs_reply) == 0 && vfs_reply.msg_type == VFS_RESP_OK {
                                let read_msg = crate::task::IpcMessage { sender: 0, msg_type: VFS_CMD_READ, length: 0, reserved: 0, payload: [0; 32] };
                                if user_ipc_send(vfs_task_id, &read_msg) == 0 && user_ipc_recv(vfs_task_id, &mut vfs_reply) == 0 && vfs_reply.msg_type == VFS_RESP_OK {
                                    let len = vfs_reply.length as usize;
                                    if let Ok(content) = core::str::from_utf8(&vfs_reply.payload[..len]) {
                                        shell_out(content, wm_task_id);
                                    }
                                }
                                let close_msg = crate::task::IpcMessage { sender: 0, msg_type: VFS_CMD_CLOSE, length: 0, reserved: 0, payload: [0; 32] };
                                user_ipc_send(vfs_task_id, &close_msg);
                                user_ipc_recv(vfs_task_id, &mut vfs_reply);
                            } else {
                                shell_out("Err: File not found", wm_task_id);
                            }
                            user_print("\n");
                        } else if cmd_str == "clear" {
                            shell_out("", wm_task_id);
                            user_print("\n");
                        } else if cmd_str == "info" {
                            shell_out("AetherV OS v1.3 - RV64 Microkernel", wm_task_id);
                            user_print("\n");
                        } else {
                            shell_out("Unknown command", wm_task_id);
                            user_print("\n");
                        }
                    }
                    cmd_len = 0;
                    user_print("aetherv-shell> ");
                } else if c == '\x08' { // Backspace
                    if cmd_len > 0 {
                        cmd_len -= 1;
                        user_print("\x08 \x08");
                    }
                } else if cmd_len < 32 {
                    cmd_buf[cmd_len] = c as u8;
                    cmd_len += 1;
                    let mut single_char = [0u8; 4];
                    if let Some(s) = c.encode_utf8(&mut single_char).get(..) {
                        user_print(s);
                    }
                }
            }
        }
        user_yield();
    }
}
