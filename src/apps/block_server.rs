// src/apps/block_server.rs
#![allow(static_mut_refs)]

use super::{user_ipc_recv, user_ipc_send, log_info, ns_register, BLOCK_CMD_READ, BLOCK_CMD_WRITE, BLOCK_RESP_OK, BLOCK_RESP_ERR};
use crate::task::{IpcMessage, IPC_WILDCARD};
use crate::drivers::virtio::find_device;

pub static mut BLOCK_DEVICE_FOUND: bool = false;

// Datos simulados de sectores (FAT16 simulado)
// Cada sector tiene 512 bytes, divididos en 16 chunks de 32 bytes.
// Definimos los chunks de los sectores clave:
// Sector 0: Boot Sector (BPB)
// Sector 1: Root Directory
// Sector 2: Data README.TXT (Cluster 2)
// Sector 3: Data CONFIG.SYS (Cluster 3)
// Sector 4: Data PERSIST.DAT (Cluster 4)

pub fn get_simulated_chunk(sector: u32, chunk: u32) -> [u8; 32] {
    let mut data = [0u8; 32];
    match sector {
        0 => {
            // Boot Sector / BPB
            if chunk == 0 {
                // Cabecera BPB básica FAT16
                data[0] = 0xEB; data[1] = 0x3C; data[2] = 0x90; // Jump
                let oem = b"AETHERV ";
                data[3..11].copy_from_slice(oem);
                data[11] = 0x00; data[12] = 0x02; // 512 bytes/sector
                data[13] = 0x01; // 1 sector/cluster
                data[14] = 0x01; data[15] = 0x00; // 1 reserved sector
            } else if chunk == 15 {
                // Firma de boot 0xAA55 al final
                data[30] = 0x55;
                data[31] = 0xAA;
            }
        }
        1 => {
            // Root Directory Table (entradas de 32 bytes cada una)
            // Cada chunk contiene una entrada de directorio FAT16 completa
            match chunk {
                0 => {
                    // README.TXT (cluster 2, tamaño 37 bytes)
                    data[0..8].copy_from_slice(b"README  ");
                    data[8..11].copy_from_slice(b"TXT");
                    data[11] = 0x20; // Archivo
                    data[26] = 2; data[27] = 0; // Cluster 2
                    let size = 37u32.to_le_bytes();
                    data[28..32].copy_from_slice(&size);
                }
                1 => {
                    // CONFIG.SYS (cluster 3, tamaño 23 bytes)
                    data[0..8].copy_from_slice(b"CONFIG  ");
                    data[8..11].copy_from_slice(b"SYS");
                    data[11] = 0x20;
                    data[26] = 3; data[27] = 0; // Cluster 3
                    let size = 23u32.to_le_bytes();
                    data[28..32].copy_from_slice(&size);
                }
                2 => {
                    // PERSIST.DAT (cluster 4, tamaño 12 bytes)
                    data[0..8].copy_from_slice(b"PERSIST ");
                    data[8..11].copy_from_slice(b"DAT");
                    data[11] = 0x20;
                    data[26] = 4; data[27] = 0; // Cluster 4
                    let size = 12u32.to_le_bytes();
                    data[28..32].copy_from_slice(&size);
                }
                _ => {}
            }
        }
        2 => {
            // README.TXT data
            if chunk == 0 {
                let text = b"AetherV OS - Persistent Block Dev\n";
                let len = text.len().min(32);
                data[0..len].copy_from_slice(&text[..len]);
            }
        }
        3 => {
            // CONFIG.SYS data
            if chunk == 0 {
                let text = b"boot_device=virtio-blk\n";
                let len = text.len().min(32);
                data[0..len].copy_from_slice(&text[..len]);
            }
        }
        4 => {
            // PERSIST.DAT data
            if chunk == 0 {
                let text = b"system_ok=1\n";
                let len = text.len().min(32);
                data[0..len].copy_from_slice(&text[..len]);
            }
        }
        _ => {}
    }
    data
}

pub fn block_server_task() {
    log_info("[Block Server] Iniciando servidor de bloques en U-Mode...\n");

    // 1. Escanear el dispositivo VirtIO Block (Device ID 2)
    let base_addr = find_device(2);
    unsafe {
        if let Some(_base) = base_addr {
            log_info("[Block Server] Disco VirtIO-Block (ID 2) detectado en base MMIO.\n");
            BLOCK_DEVICE_FOUND = true;
        } else {
            log_info("[Block Server] Disco VirtIO-Block (ID 2) no detectado. Iniciando en modo simulado.\n");
            BLOCK_DEVICE_FOUND = false;
        }
    }

    // 2. Registrar el servicio "block" en el Nameserver (ID 3)
    let registered = ns_register("block");
    if registered {
        log_info("[Block Server] Servicio 'block' registrado con exito en el Nameserver.\n");
    } else {
        log_info("[Block Server] Error al registrar servicio 'block'.\n");
    }

    loop {
        let mut msg = IpcMessage {
            sender: 0,
            msg_type: 0,
            length: 0,
            reserved: 0,
            payload: [0; 32],
        };

        let res = user_ipc_recv(IPC_WILDCARD, &mut msg);
        if res == 0 {
            match msg.msg_type {
                BLOCK_CMD_READ => {
                    // payload[0..4] -> sector ID (u32)
                    // payload[4..8] -> chunk ID (u32)
                    let mut sec_bytes = [0u8; 4];
                    sec_bytes.copy_from_slice(&msg.payload[0..4]);
                    let sector = u32::from_ne_bytes(sec_bytes);

                    let mut chk_bytes = [0u8; 4];
                    chk_bytes.copy_from_slice(&msg.payload[4..8]);
                    let chunk = u32::from_ne_bytes(chk_bytes);

                    let chunk_data = get_simulated_chunk(sector, chunk);

                    let mut reply = IpcMessage {
                        sender: 0,
                        msg_type: BLOCK_RESP_OK,
                        length: 32,
                        reserved: 0,
                        payload: [0; 32],
                    };
                    reply.payload.copy_from_slice(&chunk_data);

                    user_ipc_send(msg.sender as usize, &reply);
                }
                BLOCK_CMD_WRITE => {
                    // Simular escritura exitosa
                    let reply = IpcMessage {
                        sender: 0,
                        msg_type: BLOCK_RESP_OK,
                        length: 0,
                        reserved: 0,
                        payload: [0; 32],
                    };
                    user_ipc_send(msg.sender as usize, &reply);
                }
                _ => {
                    let reply = IpcMessage {
                        sender: 0,
                        msg_type: BLOCK_RESP_ERR,
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
