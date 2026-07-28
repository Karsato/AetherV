// src/apps/nameserver.rs
#![allow(static_mut_refs)]

use super::{user_ipc_recv, user_ipc_send, log_debug, NS_CMD_REGISTER, NS_CMD_LOOKUP, NS_RESP_SUCCESS, NS_RESP_ERROR};
use crate::task::{IpcMessage, IPC_WILDCARD};

#[derive(Copy, Clone, Debug)]
pub struct ServiceEntry {
    pub name: [u8; 16],
    pub task_id: usize,
}

pub static mut SERVICES: [Option<ServiceEntry>; 8] = [None; 8];

pub fn nameserver_task() {
    loop {
        let mut msg = IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
        let res = user_ipc_recv(IPC_WILDCARD, &mut msg);
        if res == 0 {
            log_debug("[Nameserver] Solicitud recibida!\n");
            match msg.msg_type {
                NS_CMD_REGISTER => {
                    let mut name = [0u8; 16];
                    name.copy_from_slice(&msg.payload[0..16]);
                    let task_id = msg.sender as usize;
                    let mut registered = false;
                    unsafe {
                        for entry in SERVICES.iter_mut() {
                            if let Some(e) = entry {
                                if e.name == name { e.task_id = task_id; registered = true; break; }
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
                    let reply = IpcMessage {
                        sender: 3,
                        msg_type: if registered { NS_RESP_SUCCESS } else { NS_RESP_ERROR },
                        length: 0, reserved: 0, payload: [0; 32],
                    };
                    user_ipc_send(task_id, &reply);
                }
                NS_CMD_LOOKUP => {
                    let mut name = [0u8; 16];
                    name.copy_from_slice(&msg.payload[0..16]);
                    let mut found_id = None;
                    unsafe {
                        for entry in SERVICES.iter() {
                            if let Some(e) = entry {
                                if e.name == name { found_id = Some(e.task_id); break; }
                            }
                        }
                    }
                    let mut reply = IpcMessage {
                        sender: 3,
                        msg_type: if found_id.is_some() { NS_RESP_SUCCESS } else { NS_RESP_ERROR },
                        length: 4, reserved: 0, payload: [0; 32],
                    };
                    if let Some(tid) = found_id {
                        let bytes = (tid as u32).to_ne_bytes();
                        reply.payload[16..20].copy_from_slice(&bytes);
                    }
                    user_ipc_send(msg.sender as usize, &reply);
                }
                _ => {
                    let reply = IpcMessage { sender: 3, msg_type: NS_RESP_ERROR, length: 0, reserved: 0, payload: [0; 32] };
                    user_ipc_send(msg.sender as usize, &reply);
                }
            }
        }
    }
}
