// src/apps/shell.rs

use super::{user_ipc_recv, user_ipc_send, user_print, log_info, user_yield,
            ns_lookup, make_vfs_open_msg,
            INPUT_CMD_GET_KEY, INPUT_RESP_KEY, INPUT_RESP_EMPTY,
            WM_CMD_DRAW_TEXT, WM_CMD_UPDATE,
            VFS_CMD_READ, VFS_CMD_CLOSE, VFS_RESP_OK};
use crate::task::IpcMessage;

pub fn shell_task() {
    log_info("[Shell] Iniciando consola interactiva en U-Mode...\n");

    let vfs_task_id = loop {
        if let Some(id) = ns_lookup("vfs") { break id; }
        user_yield();
    };

    let input_task_id = loop {
        if let Some(id) = ns_lookup("input") { break id; }
        user_yield();
    };

    let wm_task_id = loop {
        if let Some(id) = ns_lookup("wm") { break id; }
        user_yield();
    };

    log_info("[Shell] Servicios 'vfs', 'input' y 'wm' vinculados.\n");
    user_print("aetherv-shell> ");

    let mut cmd_buf = [0u8; 32];
    let mut cmd_len = 0usize;

    let shell_out = |s: &str, wm_id: usize| {
        user_print(s);
        let mut msg = IpcMessage {
            sender: 0,
            msg_type: WM_CMD_DRAW_TEXT,
            length: 26,
            reserved: 0,
            payload: [0; 32],
        };
        msg.payload[0] = 1;  // Window ID 1 (Keyboard Console)
        msg.payload[1] = 8;
        msg.payload[2] = 80;
        msg.payload[3] = 255; msg.payload[4] = 255; msg.payload[5] = 255;
        let bytes = s.as_bytes();
        let copy_len = bytes.len().min(20);
        msg.payload[6..6 + copy_len].copy_from_slice(&bytes[..copy_len]);
        let mut reply = IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
        user_ipc_send(wm_id, &msg);
        user_ipc_recv(wm_id, &mut reply);
    };

    loop {
        let req = IpcMessage {
            sender: 0,
            msg_type: INPUT_CMD_GET_KEY,
            length: 0,
            reserved: 0,
            payload: [0; 32],
        };

        if user_ipc_send(input_task_id, &req) == 0 {
            let mut reply = IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
            if user_ipc_recv(input_task_id, &mut reply) == 0 {
                if reply.msg_type == INPUT_RESP_KEY {
                    let c = reply.payload[0] as char;

                    // Siempre notificar al Window Manager para mantener el renderizado en sincronía
                    let mut key_msg = IpcMessage {
                        sender: 0,
                        msg_type: 2004,
                        length: 1,
                        reserved: 0,
                        payload: [0; 32],
                    };
                    key_msg.payload[0] = c as u8;
                    user_ipc_send(wm_task_id, &key_msg);

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
                                let mut vfs_reply = IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
                                if user_ipc_send(vfs_task_id, &open_msg) == 0
                                    && user_ipc_recv(vfs_task_id, &mut vfs_reply) == 0
                                    && vfs_reply.msg_type == VFS_RESP_OK
                                {
                                    let read_msg = IpcMessage { sender: 0, msg_type: VFS_CMD_READ, length: 0, reserved: 0, payload: [0; 32] };
                                    if user_ipc_send(vfs_task_id, &read_msg) == 0
                                        && user_ipc_recv(vfs_task_id, &mut vfs_reply) == 0
                                        && vfs_reply.msg_type == VFS_RESP_OK
                                    {
                                        let len = vfs_reply.length as usize;
                                        if let Ok(content) = core::str::from_utf8(&vfs_reply.payload[..len]) {
                                            shell_out(content, wm_task_id);
                                        }
                                    }
                                    let close_msg = IpcMessage { sender: 0, msg_type: VFS_CMD_CLOSE, length: 0, reserved: 0, payload: [0; 32] };
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
                                shell_out("AetherV OS v1.8 - RV64 Microkernel", wm_task_id);
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
                } else if reply.msg_type == INPUT_RESP_EMPTY {
                    user_yield();
                }
            } else {
                user_yield();
            }
        } else {
            user_yield();
        }
    }
}
