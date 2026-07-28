// src/apps/gpu_server.rs

use super::{user_ipc_recv, user_ipc_send, log_info, log_debug, user_print,
            ns_register, str_to_u8_16, NS_RESP_SUCCESS};
use crate::task::{IpcMessage, IPC_WILDCARD};
use crate::drivers;

pub fn gpu_driver_server() {
    log_info("[GPU Server] Iniciando inicialización en U-Mode...\n");
    drivers::gpu::init();
    log_info("[GPU Server] Inicialización completada con éxito.\n");

    log_info("[GPU Server] Registrando servicio 'display' en el Nameserver (Tarea 3)...\n");
    let mut reg_msg = IpcMessage {
        sender: 0,
        msg_type: super::NS_CMD_REGISTER,
        length: 16,
        reserved: 0,
        payload: [0; 32],
    };
    reg_msg.payload[0..16].copy_from_slice(&str_to_u8_16("display"));

    let mut res = user_ipc_send(3, &reg_msg);
    if res == 0 {
        let mut reply = IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
        res = user_ipc_recv(3, &mut reply);
        if res == 0 && reply.msg_type == NS_RESP_SUCCESS {
            log_info("[GPU Server] Registro exitoso en el Nameserver!\n");
        } else {
            user_print("[GPU Server] Error en el registro en el Nameserver.\n");
        }
    } else {
        user_print("[GPU Server] Error al conectar con el Nameserver.\n");
    }

    log_info("[GPU Server] Entrando en bucle de servicio IPC...\n");

    loop {
        #[allow(unused_mut)]
        let mut msg = IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
        let mut res: isize;
        unsafe {
            core::arch::asm!(
                "ecall",
                in("a7") 5usize,
                inout("a0") crate::task::IPC_WILDCARD => res,
                in("a1") &msg as *const _ as usize,
                clobber_abi("C"),
            );
        }

        if res == 0 {
            log_debug("[GPU Server] Solicitud recibida!\n");
            match msg.msg_type {
                1 => {
                    drivers::gpu::draw_pattern();
                    drivers::gpu::flush_screen(0x10008000);
                    msg.msg_type = 200;
                }
                2 => {
                    let r = msg.payload[0] as u32;
                    let g = msg.payload[1] as u32;
                    let b = msg.payload[2] as u32;
                    unsafe {
                        let fb = &mut drivers::gpu::FRAMEBUFFER;
                        let color_val = 0xFF000000 | (r << 16) | (g << 8) | b;
                        for pixel in fb.pixels.iter_mut() {
                            *pixel = color_val;
                        }
                    }
                    drivers::gpu::flush_screen(0x10008000);
                    msg.msg_type = 200;
                }
                3 => {
                    // GPU_CMD_FLUSH — usado por WM
                    drivers::gpu::flush_screen(0x10008000);
                    msg.msg_type = 200;
                }
                _ => {
                    log_debug("[GPU Server] Comando desconocido\n");
                    msg.msg_type = 404;
                }
            }
            unsafe {
                core::arch::asm!(
                    "ecall",
                    in("a7") 4usize,
                    inout("a0") msg.sender as usize => _,
                    in("a1") &msg as *const _ as usize,
                    clobber_abi("C"),
                );
            }
        }
    }
}
