// src/apps/vfs_server.rs
#![allow(static_mut_refs)]

use super::{user_ipc_recv, user_ipc_send, log_info, user_yield, ns_lookup,
            NS_CMD_REGISTER, NS_RESP_SUCCESS,
            VFS_CMD_OPEN, VFS_CMD_READ, VFS_CMD_CLOSE,
            VFS_RESP_OK, VFS_RESP_ERR,
            BLOCK_CMD_READ, BLOCK_RESP_OK};
use crate::task::{IpcMessage, IPC_WILDCARD, MAX_TASKS};
use super::str_to_u8_16;

#[derive(Clone, Copy, Debug)]
pub struct OpenFile {
    pub cluster: u16,
    pub size: u32,
}

pub static mut OPEN_FILES: [Option<OpenFile>; MAX_TASKS] = [None; MAX_TASKS];

// Utilidad para formatear un path corto como "/readme.txt" a la estructura FAT16 de 11 bytes "README  TXT"
fn path_to_fat16(path: &[u8]) -> [u8; 11] {
    let mut fatname = [b' '; 11];
    
    // Omitir '/' inicial si existe
    let start = if path.len() > 0 && path[0] == b'/' { 1 } else { 0 };
    let mut dot_idx = None;
    for i in start..path.len() {
        if path[i] == b'.' {
            dot_idx = Some(i);
            break;
        }
    }
    
    let name_end = dot_idx.unwrap_or(path.len());
    
    // Copiar nombre (hasta 8 caracteres)
    let mut dst = 0;
    for src in start..name_end {
        if dst < 8 {
            let c = path[src];
            // Convertir a mayúsculas
            fatname[dst] = if c >= b'a' && c <= b'z' { c - 32 } else { c };
            dst += 1;
        }
    }
    
    // Copiar extensión (hasta 3 caracteres)
    if let Some(dot) = dot_idx {
        let mut ext_dst = 8;
        for src in (dot + 1)..path.len() {
            if ext_dst < 11 {
                let c = path[src];
                fatname[ext_dst] = if c >= b'a' && c <= b'z' { c - 32 } else { c };
                ext_dst += 1;
            }
        }
    }
    
    fatname
}

pub fn vfs_server() {
    log_info("[VFS Server] Iniciando en U-Mode...\n");

    // 1. Registrar el servicio "vfs" en el Nameserver
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

    // 2. Vincular el servicio "block"
    let block_task_id = loop {
        if let Some(id) = ns_lookup("block") {
            log_info("[VFS Server] Servicio de bloques 'block' vinculado correctamente.\n");
            break id;
        }
        user_yield();
    };

    log_info("[VFS Server] Entrando en bucle de servicio IPC de archivos (FAT16)...\n");

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
                    
                    let fat16_target = path_to_fat16(requested_path);
                    let mut found_file = None;

                    // Escanear directorio raíz (Sector 1) pidiendo chunks al block_server
                    for chunk_idx in 0..4 {
                        let mut block_req = IpcMessage {
                            sender: 0,
                            msg_type: BLOCK_CMD_READ,
                            length: 8,
                            reserved: 0,
                            payload: [0; 32],
                        };
                        // payload[0..4] = Sector 1 (Root Directory)
                        block_req.payload[0..4].copy_from_slice(&1u32.to_ne_bytes());
                        // payload[4..8] = Chunk ID
                        block_req.payload[4..8].copy_from_slice(&(chunk_idx as u32).to_ne_bytes());

                        let mut block_reply = IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
                        
                        if user_ipc_send(block_task_id, &block_req) == 0 
                            && user_ipc_recv(block_task_id, &mut block_reply) == 0
                            && block_reply.msg_type == BLOCK_RESP_OK 
                        {
                            // Verificar si coincide la cabecera FAT16 de 11 bytes
                            let entry_name = &block_reply.payload[0..11];
                            if entry_name == fat16_target {
                                // Encontrado! Extraer starting cluster (bytes 26..27) y size (bytes 28..32)
                                let mut clust_bytes = [0u8; 2];
                                clust_bytes.copy_from_slice(&block_reply.payload[26..28]);
                                let cluster = u16::from_le_bytes(clust_bytes);

                                let mut size_bytes = [0u8; 4];
                                size_bytes.copy_from_slice(&block_reply.payload[28..32]);
                                let size = u32::from_le_bytes(size_bytes);

                                found_file = Some(OpenFile { cluster, size });
                                break;
                            }
                        }
                    }

                    let mut reply = IpcMessage { sender: 7, msg_type: VFS_RESP_ERR, length: 0, reserved: 0, payload: [0; 32] };
                    if let Some(file_info) = found_file {
                        unsafe {
                            OPEN_FILES[client_id] = Some(file_info);
                        }
                        reply.msg_type = VFS_RESP_OK;
                        reply.length = 4;
                        reply.payload[0..4].copy_from_slice(&file_info.size.to_ne_bytes());
                        log_info("[VFS Server] Archivo FAT16 encontrado y abierto.\n");
                    } else {
                        log_info("[VFS Server] Archivo no encontrado en disco FAT16.\n");
                    }
                    user_ipc_send(client_id, &reply);
                }
                VFS_CMD_READ => {
                    let mut reply = IpcMessage { sender: 7, msg_type: VFS_RESP_ERR, length: 0, reserved: 0, payload: [0; 32] };
                    let open_info = unsafe { OPEN_FILES[client_id] };
                    
                    if let Some(info) = open_info {
                        // El cluster corresponde directamente al sector de datos (para simplificar la simulación)
                        let sector_to_read = info.cluster as u32;
                        
                        let mut block_req = IpcMessage {
                            sender: 0,
                            msg_type: BLOCK_CMD_READ,
                            length: 8,
                            reserved: 0,
                            payload: [0; 32],
                        };
                        block_req.payload[0..4].copy_from_slice(&sector_to_read.to_ne_bytes());
                        block_req.payload[4..8].copy_from_slice(&0u32.to_ne_bytes()); // Chunk 0

                        let mut block_reply = IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
                        
                        if user_ipc_send(block_task_id, &block_req) == 0 
                            && user_ipc_recv(block_task_id, &mut block_reply) == 0
                            && block_reply.msg_type == BLOCK_RESP_OK 
                        {
                            reply.msg_type = VFS_RESP_OK;
                            reply.length = info.size.min(32);
                            reply.payload.copy_from_slice(&block_reply.payload);
                        }
                    }
                    user_ipc_send(client_id, &reply);
                }
                VFS_CMD_CLOSE => {
                    unsafe {
                        OPEN_FILES[client_id] = None;
                    }
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
