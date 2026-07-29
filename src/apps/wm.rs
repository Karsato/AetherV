    // src/apps/wm.rs
    #![allow(static_mut_refs)]
    
    use super::{user_ipc_recv, user_ipc_send, user_print, log_info, user_yield,
                ns_lookup, str_to_u8_16,
                NS_CMD_REGISTER, NS_RESP_SUCCESS,
                WM_CMD_CREATE_WINDOW, WM_CMD_DRAW_RECT, WM_CMD_DRAW_TEXT, WM_CMD_UPDATE,
                WM_RESP_SUCCESS, WM_RESP_ERROR};
    use crate::task::{IpcMessage, IPC_WILDCARD};
    use crate::drivers;
    
    #[derive(Copy, Clone, Debug)]
    pub struct Window {
        pub id: usize,
        pub x: usize, pub y: usize,
        pub w: usize, pub h: usize,
        pub title: [u8; 16],
        pub bg_color: u32,
        pub active: bool,
    }
    
    pub static mut WINDOWS: [Option<Window>; 4] = [None; 4];
    pub static mut CONSOLE_BUFFER: [u8; 128] = [0; 128];
    pub static mut CONSOLE_LEN: usize = 0;
    pub static mut SHELL_OUTPUT: [u8; 64] = [0; 64];
    pub static mut SHELL_OUTPUT_LEN: usize = 0;
    
    pub fn wm_composite(gpu_task_id: usize) {
        drivers::gpu::draw_rect(0, 0, 640, 480, 0xFF1A1B26);
        drivers::gpu::draw_rect(0, 0, 640, 24, 0xFF16161E);
        drivers::gpu::draw_string(10, 8, "AetherV OS  |  U-Mode Window Manager  |  Tasks: 7  |  Idle CPU: 0%", 0xFFC0CAF5);
    
        unsafe {
            for idx in 0..4 {
                if let Some(ref w) = WINDOWS[idx] {
                    drivers::gpu::draw_rect(w.x + 3, w.y + 3, w.w, w.h, 0xFF0D0E15);
                    drivers::gpu::draw_rect(w.x, w.y, w.w, w.h, if w.active { 0xFF7AA2F7 } else { 0xFF565F89 });
                    drivers::gpu::draw_rect(w.x + 2, w.y + 2, w.w - 4, 18, if w.active { 0xFF3D59A1 } else { 0xFF24283B });
                    drivers::gpu::draw_rect(w.x + 2, w.y + 20, w.w - 4, w.h - 22, w.bg_color);
    
                    let mut len = 0;
                    while len < 16 && w.title[len] != 0 { len += 1; }
                    if let Ok(title_str) = core::str::from_utf8(&w.title[0..len]) {
                        drivers::gpu::draw_string(w.x + 8, w.y + 6, title_str, 0xFFFFFFFF);
                    }
                    drivers::gpu::draw_string(w.x + w.w - 20, w.y + 6, "x", 0xFFF7768E);
    
                    if w.id == 0 {
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
                                    7 => "vfs_server      ",
                                    8 => "vfs_client      ",
                                    9 => "shell_task      ",
                                    _ => "unknown_task    ",
                                };
                                let status_str = match t.status {
                                    crate::task::TaskStatus::Ready      => "Ready      ",
                                    crate::task::TaskStatus::Running    => "Running    ",
                                    crate::task::TaskStatus::BlockedSend => "BlockedSend",
                                    crate::task::TaskStatus::BlockedRecv => "BlockedRecv",
                                    crate::task::TaskStatus::Exited     => "Exited     ",
                                    _ => "Unused     ",
                                };
                                let mut line_buf = [b' '; 32];
                                line_buf[0] = b'0' + (i as u8);
                                for j in 0..16 { line_buf[4 + j] = name.as_bytes()[j]; }
                                for j in 0..11 { line_buf[21 + j] = status_str.as_bytes()[j]; }
                                if let Ok(s) = core::str::from_utf8(&line_buf[0..32]) {
                                    drivers::gpu::draw_string(w.x + 8, y_offset, s, 0xFFC0CAF5);
                                }
                                y_offset += 12;
                            }
                        }
                    } else if w.id == 1 {
                        let mut y_offset = w.y + 26;
                        drivers::gpu::draw_string(w.x + 8, y_offset, "Interactive OS Shell (U-Mode)", 0xFF2AC3DE);
                        y_offset += 14;
                        drivers::gpu::draw_string(w.x + 8, y_offset, "Type command (e.g. help):", 0xFF565F89);
                        y_offset += 14;
    
                        let mut line_buf = [b' '; 40];
                        line_buf[0] = b'>';
                        line_buf[1] = b' ';
                        let len = CONSOLE_LEN;
                        for j in 0..len { line_buf[2 + j] = CONSOLE_BUFFER[j]; }
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
    
        let flush_msg = IpcMessage { sender: 0, msg_type: 3, length: 0, reserved: 0, payload: [0; 32] };
        user_ipc_send(gpu_task_id, &flush_msg);
        let mut reply = IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
        user_ipc_recv(gpu_task_id, &mut reply);
    }
    
    pub fn window_manager_task() {
        log_info("[WM] Buscando el servicio 'display' en el Nameserver (Tarea 3)...\n");
    
        let gpu_task_id = loop {
            if let Some(id) = ns_lookup("display") {
                log_info("[WM] Servicio 'display' resuelto con éxito.\n");
                break id;
            }
            super::user_yield();
        };
    
        log_info("[WM] Registrando servicio 'wm' en el Nameserver (Tarea 3)...\n");
        let mut reg_msg = IpcMessage {
            sender: 0,
            msg_type: NS_CMD_REGISTER,
            length: 16,
            reserved: 0,
            payload: [0; 32],
        };
        reg_msg.payload[0..16].copy_from_slice(&str_to_u8_16("wm"));
        let mut res = user_ipc_send(3, &reg_msg);
        if res == 0 {
            let mut reply = IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
            res = user_ipc_recv(3, &mut reply);
            if res == 0 && reply.msg_type == NS_RESP_SUCCESS {
                log_info("[WM] Registro de 'wm' exitoso en el Nameserver!\n");
            } else {
                user_print("[WM] Error en el registro de 'wm'.\n");
            }
        } else {
            user_print("[WM] Error al conectar con el Nameserver.\n");
        }
    
        unsafe {
            let mut title0 = [0u8; 16];
            title0[0..14].copy_from_slice(b"System Monitor");
            WINDOWS[0] = Some(Window { id: 0, x: 20, y: 35, w: 290, h: 200, title: title0, bg_color: 0xFF1F2335, active: false });
    
            let mut title1 = [0u8; 16];
            title1[0..16].copy_from_slice(b"Keyboard Console");
            WINDOWS[1] = Some(Window { id: 1, x: 330, y: 35, w: 290, h: 200, title: title1, bg_color: 0xFF1A1B26, active: true });
        }
    
        // Renderizado inicial explícito
        wm_composite(gpu_task_id);
        user_yield();
        log_info("[WM] Entrando en bucle de servicio de ventanas...\n");
    
        loop {
            let mut msg = IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
            let res = user_ipc_recv(IPC_WILDCARD, &mut msg);
            if res == 0 {
                match msg.msg_type {
                    2004 => {
                        let c = msg.payload[0] as char;
                        unsafe {
                            if let Some(ref mut win0) = WINDOWS[0] { win0.active = false; }
                            if let Some(ref mut win1) = WINDOWS[1] { win1.active = true; }
    
                            if c == '\x08' {
                                if CONSOLE_LEN > 0 { CONSOLE_LEN -= 1; }
                            } else if c == '\n' || c == '\r' {
                                let mut cmd = [0u8; 32];
                                let len = core::cmp::min(CONSOLE_LEN, 32);
                                for i in 0..len { cmd[i] = CONSOLE_BUFFER[i]; }
                                if len >= 4 && &cmd[0..4] == b"help" {
                                    let resp = b"Cmds: help, ls, cat <file>, clear, info";
                                    SHELL_OUTPUT[0..resp.len()].copy_from_slice(resp);
                                    SHELL_OUTPUT_LEN = resp.len();
                                } else if len >= 2 && &cmd[0..2] == b"ls" {
                                    let resp = b"/readme.txt  /config.sys";
                                    SHELL_OUTPUT[0..resp.len()].copy_from_slice(resp);
                                    SHELL_OUTPUT_LEN = resp.len();
                                } else if len >= 4 && &cmd[0..4] == b"cat " {
                                    let resp = b"AetherV OS System File Content";
                                    SHELL_OUTPUT[0..resp.len()].copy_from_slice(resp);
                                    SHELL_OUTPUT_LEN = resp.len();
                                } else if len >= 5 && &cmd[0..5] == b"clear" {
                                    SHELL_OUTPUT_LEN = 0;
                                } else if len >= 4 && &cmd[0..4] == b"info" {
                                    let resp = b"AetherV OS v1.8 - RV64 Microkernel";
                                    SHELL_OUTPUT[0..resp.len()].copy_from_slice(resp);
                                    SHELL_OUTPUT_LEN = resp.len();
                                } else {
                                    let resp = b"Unknown command";
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
                        let r = msg.payload[20] as u32;
                        let g = msg.payload[21] as u32;
                        let b = msg.payload[22] as u32;
                        let bg_color = 0xFF000000 | (r << 16) | (g << 8) | b;
                        let mut found_idx = None;
                        unsafe {
                            for idx in 2..4 {
                                if WINDOWS[idx].is_none() {
                                    let (wx, wy, ww, wh) = if idx == 2 {
                                        (20, 250, 290, 200)
                                    } else {
                                        (330, 250, 290, 200)
                                    };
                                    WINDOWS[idx] = Some(Window { id: idx, x: wx, y: wy, w: ww, h: wh, title, bg_color, active: false });
                                    found_idx = Some(idx);
                                    break;
                                }
                            }
                        }
                        let mut reply = IpcMessage {
                            sender: 2,
                            msg_type: if found_idx.is_some() { WM_RESP_SUCCESS } else { WM_RESP_ERROR },
                            length: 4, reserved: 0, payload: [0; 32],
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
                        let rx = msg.payload[1] as usize; let ry = msg.payload[2] as usize;
                        let rw = msg.payload[3] as usize; let rh = msg.payload[4] as usize;
                        let r = msg.payload[5] as u32; let g = msg.payload[6] as u32; let b = msg.payload[7] as u32;
                        let color = 0xFF000000 | (r << 16) | (g << 8) | b;
                        unsafe {
                            if win_id < 4 {
                                if let Some(ref win) = WINDOWS[win_id] {
                                    let px = win.x + 2 + rx; let py = win.y + 20 + ry;
                                    let max_w = win.w - 4; let max_h = win.h - 22;
                                    let draw_w = if rx + rw > max_w { max_w.saturating_sub(rx) } else { rw };
                                    let draw_h = if ry + rh > max_h { max_h.saturating_sub(ry) } else { rh };
                                    drivers::gpu::draw_rect(px, py, draw_w, draw_h, color);
                                }
                            }
                        }
                        let reply = IpcMessage { sender: 2, msg_type: WM_RESP_SUCCESS, length: 0, reserved: 0, payload: [0; 32] };
                        user_ipc_send(msg.sender as usize, &reply);
                    }
                    WM_CMD_DRAW_TEXT => {
                        let win_id = msg.payload[0] as usize;
                        let tx = msg.payload[1] as usize; let ty = msg.payload[2] as usize;
                        let r = msg.payload[3] as u32; let g = msg.payload[4] as u32; let b = msg.payload[5] as u32;
                        let color = 0xFF000000 | (r << 16) | (g << 8) | b;
                        let mut text_buf = [0u8; 20];
                        text_buf.copy_from_slice(&msg.payload[6..26]);
                        unsafe {
                            if win_id < 4 {
                                if let Some(ref win) = WINDOWS[win_id] {
                                    let px = win.x + 2 + tx; let py = win.y + 20 + ty;
                                    let mut len = 0;
                                    while len < 20 && text_buf[len] != 0 { len += 1; }
                                    if let Ok(s) = core::str::from_utf8(&text_buf[0..len]) {
                                        drivers::gpu::draw_string(px, py, s, color);
                                    }
                                }
                            }
                        }
                        let reply = IpcMessage { sender: 2, msg_type: WM_RESP_SUCCESS, length: 0, reserved: 0, payload: [0; 32] };
                        user_ipc_send(msg.sender as usize, &reply);
                    }
                    WM_CMD_UPDATE => {
                        let reply = IpcMessage { sender: 2, msg_type: WM_RESP_SUCCESS, length: 0, reserved: 0, payload: [0; 32] };
                        user_ipc_send(msg.sender as usize, &reply);
                        let flush_msg = IpcMessage { sender: 0, msg_type: 3, length: 0, reserved: 0, payload: [0; 32] };
                        user_ipc_send(gpu_task_id, &flush_msg);
                        let mut reply_gpu = IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
                        user_ipc_recv(gpu_task_id, &mut reply_gpu);
                    }
                    _ => {
                        let reply = IpcMessage { sender: 2, msg_type: WM_RESP_ERROR, length: 0, reserved: 0, payload: [0; 32] };
                        user_ipc_send(msg.sender as usize, &reply);
                    }
                }
            }
        }
    }

