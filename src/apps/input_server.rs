// src/apps/input_server.rs
#![allow(static_mut_refs)]

use super::{user_ipc_recv, user_ipc_send, user_print, log_info, log_debug,
            ns_lookup, str_to_u8_16,
            NS_CMD_REGISTER, NS_RESP_SUCCESS, NS_RESP_ERROR,
            INPUT_CMD_GET_KEY, INPUT_RESP_KEY, INPUT_RESP_EMPTY};
use crate::task::{IpcMessage, IPC_WILDCARD, IPC_SENDER_NOTIFICATION};
use crate::drivers;

// Cola circular de teclas
pub static mut KEY_BUFFER: [char; 64] = ['\0'; 64];
static mut KEY_HEAD: usize = 0;
static mut KEY_TAIL: usize = 0;

pub fn push_key(c: char) {
    unsafe {
        let next = (KEY_HEAD + 1) % 64;
        if next != KEY_TAIL {
            KEY_BUFFER[KEY_HEAD] = c;
            KEY_HEAD = next;
        }
    }
}

pub fn pop_key() -> Option<char> {
    unsafe {
        if KEY_TAIL == KEY_HEAD { None }
        else {
            let c = KEY_BUFFER[KEY_TAIL];
            KEY_TAIL = (KEY_TAIL + 1) % 64;
            Some(c)
        }
    }
}

pub fn keycode_to_char(code: u16) -> Option<char> {
    match code {
        2  => Some('1'), 3  => Some('2'), 4  => Some('3'), 5  => Some('4'),
        6  => Some('5'), 7  => Some('6'), 8  => Some('7'), 9  => Some('8'),
        10 => Some('9'), 11 => Some('0'), 12 => Some('-'), 13 => Some('='),
        14 => Some('\x08'), // Backspace
        15 => Some('\t'),
        16 => Some('q'), 17 => Some('w'), 18 => Some('e'), 19 => Some('r'),
        20 => Some('t'), 21 => Some('y'), 22 => Some('u'), 23 => Some('i'),
        24 => Some('o'), 25 => Some('p'), 26 => Some('['), 27 => Some(']'),
        28 => Some('\n'), // Enter
        30 => Some('a'), 31 => Some('s'), 32 => Some('d'), 33 => Some('f'),
        34 => Some('g'), 35 => Some('h'), 36 => Some('j'), 37 => Some('k'),
        38 => Some('l'), 39 => Some(';'), 40 => Some('\''), 41 => Some('`'),
        44 => Some('z'), 45 => Some('x'), 46 => Some('c'), 47 => Some('v'),
        48 => Some('b'), 49 => Some('n'), 50 => Some('m'), 51 => Some(','),
        52 => Some('.'), 53 => Some('/'),
        57 => Some(' '),
        _ => None,
    }
}

pub fn input_driver_server() {
    log_info("[Input Server] Iniciando inicialización en U-Mode...\n");
    drivers::input::init();
    log_info("[Input Server] Inicialización completada con éxito.\n");

    log_info("[Input Server] Registrando servicio 'input' en el Nameserver (Tarea 3)...\n");
    let mut reg_msg = IpcMessage {
        sender: 0,
        msg_type: NS_CMD_REGISTER,
        length: 16,
        reserved: 0,
        payload: [0; 32],
    };
    reg_msg.payload[0..16].copy_from_slice(&str_to_u8_16("input"));

    let mut res = user_ipc_send(3, &reg_msg);
    if res == 0 {
        let mut reply = IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
        res = user_ipc_recv(3, &mut reply);
        if res == 0 && reply.msg_type == NS_RESP_SUCCESS {
            log_info("[Input Server] Registro exitoso en el Nameserver!\n");
        } else {
            user_print("[Input Server] Error en el registro en el Nameserver.\n");
        }
    } else {
        user_print("[Input Server] Error al conectar con el Nameserver.\n");
    }

    log_info("[Input Server] Entrando en bucle de servicio IPC...\n");

    loop {
        let mut msg = IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
        let res = user_ipc_recv(IPC_WILDCARD, &mut msg);
        if res == 0 {
            if msg.sender == IPC_SENDER_NOTIFICATION {
                drivers::input::process_events(|event| {
                    if event.event_type == 1 && (event.value == 1 || event.value == 2) {
                        if let Some(c) = keycode_to_char(event.code) {
                            log_debug("[Input Server] Tecla detectada\n");

                            // Lookup WM y notificar tecla
                            let mut wm_task_id = 0;
                            if let Some(id) = ns_lookup("wm") {
                                wm_task_id = id;
                            }
                            if wm_task_id != 0 {
                                let mut wm_msg = IpcMessage {
                                    sender: 0,
                                    msg_type: 2004,
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
                match msg.msg_type {
                    INPUT_CMD_GET_KEY => {
                        let mut reply = IpcMessage {
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
                        let reply = IpcMessage {
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
