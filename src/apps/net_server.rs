// src/apps/net_server.rs
#![allow(static_mut_refs)]

use super::{user_ipc_recv, user_ipc_send, log_info, ns_register, NET_CMD_STATUS, NET_CMD_GET_HTTP, NET_RESP_OK, NET_RESP_ERR};
use crate::task::{IpcMessage, IPC_WILDCARD};
use crate::drivers::virtio::find_device;

pub static mut PACKETS_RX: u32 = 142;
pub static mut PACKETS_TX: u32 = 88;
pub static mut NET_DEVICE_FOUND: bool = false;

pub fn net_server_task() {
    log_info("[Net Server] Iniciando servidor de red en U-Mode...\n");

    // 1. Escanear el dispositivo VirtIO Network (Device ID 1)
    let base_addr = find_device(1);
    unsafe {
        if let Some(_base) = base_addr {
            log_info("[Net Server] Tarjeta de red VirtIO-Net (ID 1) detectada en base MMIO: ");
            // Convert physical base to string/hex representation (or just print confirmation)
            NET_DEVICE_FOUND = true;
        } else {
            log_info("[Net Server] Tarjeta de red VirtIO-Net (ID 1) no detectada. Iniciando en modo simulado.\n");
            NET_DEVICE_FOUND = false;
        }
    }

    // 2. Registrar el servicio "net" en el Nameserver (ID 3)
    let registered = ns_register("net");
    if registered {
        log_info("[Net Server] Servicio 'net' registrado con exito en el Nameserver.\n");
    } else {
        log_info("[Net Server] Error al registrar servicio 'net' en el Nameserver.\n");
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
                NET_CMD_STATUS => {
                    // Incrementar paquete recibido por la solicitud de estado
                    unsafe {
                        PACKETS_RX += 1;
                        PACKETS_TX += 1;
                    }

                    let mut reply = IpcMessage {
                        sender: 0, // Se sobreescribe por el kernel
                        msg_type: NET_RESP_OK,
                        length: 32,
                        reserved: 0,
                        payload: [0; 32],
                    };

                    // Copiar IP ficticia "192.168.1.10"
                    let ip = b"192.168.1.10\0\0\0\0";
                    reply.payload[0..16].copy_from_slice(ip);

                    // Escribir rx y tx
                    unsafe {
                        reply.payload[16..20].copy_from_slice(&PACKETS_RX.to_ne_bytes());
                        reply.payload[20..24].copy_from_slice(&PACKETS_TX.to_ne_bytes());
                        // Device found status (1 o 0)
                        reply.payload[24] = if NET_DEVICE_FOUND { 1 } else { 0 };
                    }

                    user_ipc_send(msg.sender as usize, &reply);
                }
                NET_CMD_GET_HTTP => {
                    unsafe {
                        PACKETS_RX += 1;
                        PACKETS_TX += 1;
                    }

                    let mut reply = IpcMessage {
                        sender: 0,
                        msg_type: NET_RESP_OK,
                        length: 32,
                        reserved: 0,
                        payload: [0; 32],
                    };

                    // Responder con una cabecera de monitor HTTP minimalista
                    let http_data = b"HTTP/1.1 200 OK\nSrv: AetherV-Web";
                    reply.payload[0..32].copy_from_slice(&http_data[0..32]);

                    user_ipc_send(msg.sender as usize, &reply);
                }
                _ => {
                    let reply = IpcMessage {
                        sender: 0,
                        msg_type: NET_RESP_ERR,
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
