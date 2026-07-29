// src/apps/input_server.rs
#![allow(static_mut_refs)]

use super::{user_ipc_recv, user_ipc_send, user_print, log_info, log_debug,
            str_to_u8_16,
            NS_CMD_REGISTER, NS_RESP_SUCCESS, NS_RESP_ERROR,
            INPUT_CMD_GET_KEY, INPUT_RESP_KEY, INPUT_RESP_EMPTY};
use crate::task::{IpcMessage, IPC_WILDCARD, IPC_SENDER_NOTIFICATION};
use crate::drivers;

pub static mut KEY_BUFFER: [char; 64] = ['\0'; 64];
static mut KEY_HEAD: usize = 0;
static mut KEY_TAIL: usize = 0;
static mut SHIFT_PRESSED: bool = false;

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

pub fn keycode_to_char(code: u16, shift: bool) -> Option<char> {
    const NORMAL_MAP: [char; 58] = [
        '\0', '\0', '1',  '2',  '3',  '4',  '5',  '6',  '7',  '8',  '9',  '0',  '-',  '=', 
        '\x08', '\t', 'q',  'w',  'e',  'r',  't',  'y',  'u',  'i',  'o',  'p',  '[',  ']', 
        '\n', '\0', 'a',  's',  'd',  'f',  'g',  'h',  'j',  'k',  'l',  ';',  '\'', '`', 
        '\0', '\0', 'z',  'x',  'c',  'v',  'b',  'n',  'm',  ',',  '.',  '/',  '\0', '\0', 
        '\0', ' '
    ];

    const SHIFT_MAP: [char; 58] = [
        '\0', '\0', '!',  '@',  '#',  '$',  '%',  '^',  '/',  '*',  '(',  '=',  '_',  '+', 
        '\x08', '\t', 'Q',  'W',  'E',  'R',  'T',  'Y',  'U',  'I',  'O',  'P',  '{',  '}', 
        '\n', '\0', 'A',  'S',  'D',  'F',  'G',  'H',  'J',  'K',  'L',  ':',  '"',  '~', 
        '\0', '\0', 'Z',  'X',  'C',  'V',  'B',  'N',  'M',  '<',  '>',  '?',  '\0', '\0', 
        '\0', ' '
    ];

    let idx = code as usize;
    if idx < 58 {
        let c = if shift { SHIFT_MAP[idx] } else { NORMAL_MAP[idx] };
        if c != '\0' {
            return Some(c);
        }
    }
    None
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
                    if event.event_type == 1 {
                        if event.code == 42 || event.code == 54 {
                            unsafe {
                                SHIFT_PRESSED = event.value == 1 || event.value == 2;
                            }
                        }
                        if event.value == 1 || event.value == 2 {
                            if let Some(c) = keycode_to_char(event.code, unsafe { SHIFT_PRESSED }) {
                                log_debug("[Input Server] Tecla detectada y encolada.\n");
                                push_key(c);
                            }
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
