// src/apps/vfs_server.rs

use super::{user_ipc_recv, user_ipc_send, user_print, log_info, user_yield,
            make_vfs_open_msg, make_vfs_read_resp,
            NS_CMD_REGISTER, NS_RESP_SUCCESS,
            VFS_CMD_OPEN, VFS_CMD_READ, VFS_CMD_CLOSE,
            VFS_RESP_OK, VFS_RESP_ERR};
use crate::task::{IpcMessage, IPC_WILDCARD, MAX_TASKS};
use super::str_to_u8_16;

pub struct RamFile {
    pub path: &'static [u8],
    pub content: &'static [u8],
}

pub static RAM_DISK: [RamFile; 2] = [
    RamFile { path: b"/readme.txt", content: b"AetherV OS - Microkernel RISC-V\n" },
    RamFile { path: b"/config.sys", content: b"version=1.2-alpha\n" },
];

pub fn vfs_server() {
    log_info("[VFS Server] Iniciando en U-Mode...\n");

    let mut reg_msg = IpcMessage {
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
            let mut reply = IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
            let res2 = user_ipc_recv(3, &mut reply);
            if res2 == 0 && reply.msg_type == NS_RESP_SUCCESS {
                log_info("[VFS Server] Registro 'vfs' exitoso en Nameserver!\n");
                break;
            }
        }
        user_yield();
    }

    log_info("[VFS Server] Entrando en bucle de servicio IPC...\n");

    let mut open_file_idx: [Option<usize>; MAX_TASKS] = [None; MAX_TASKS];

    loop {
        let mut msg = IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
        let res = user_ipc_recv(IPC_WILDCARD, &mut msg);
        if res == 0 {
            let client_id = msg.sender as usize;
            match msg.msg_type {
                VFS_CMD_OPEN => {
                    let mut path_len = 0;
                    while path_len < 32 && msg.payload[path_len] != 0 { path_len += 1; }
                    let requested_path = &msg.payload[..path_len];
                    let mut found = None;
                    for (idx, file) in RAM_DISK.iter().enumerate() {
                        if file.path == requested_path { found = Some(idx); break; }
                    }
                    let mut reply = IpcMessage { sender: 7, msg_type: VFS_RESP_ERR, length: 0, reserved: 0, payload: [0; 32] };
                    if let Some(file_idx) = found {
                        open_file_idx[client_id] = Some(file_idx);
                        reply.msg_type = VFS_RESP_OK;
                        let size_bytes = (RAM_DISK[file_idx].content.len() as u32).to_ne_bytes();
                        reply.payload[0..4].copy_from_slice(&size_bytes);
                        reply.length = 4;
                        log_info("[VFS Server] Archivo encontrado y abierto.\n");
                    } else {
                        user_print("[VFS Server] Archivo no encontrado.\n");
                    }
                    user_ipc_send(client_id, &reply);
                }
                VFS_CMD_READ => {
                    let mut reply = IpcMessage { sender: 7, msg_type: VFS_RESP_ERR, length: 0, reserved: 0, payload: [0; 32] };
                    if let Some(file_idx) = open_file_idx[client_id] {
                        reply = make_vfs_read_resp(RAM_DISK[file_idx].content);
                        reply.sender = 7;
                    }
                    user_ipc_send(client_id, &reply);
                }
                VFS_CMD_CLOSE => {
                    open_file_idx[client_id] = None;
                    let reply = IpcMessage { sender: 7, msg_type: VFS_RESP_OK, length: 0, reserved: 0, payload: [0; 32] };
                    user_ipc_send(client_id, &reply);
                }
                _ => {
                    let reply = IpcMessage { sender: 7, msg_type: VFS_RESP_ERR, length: 0, reserved: 0, payload: [0; 32] };
                    user_ipc_send(client_id, &reply);
                }
            }
        }
    }
}
