// src/apps/mod.rs — Fase 8: Módulo de espacio de usuario (U-Mode)

pub mod nameserver;
pub mod gpu_server;
pub mod input_server;
pub mod wm;
pub mod vfs_server;
pub mod shell;
pub mod clients;

// ---------------------------------------------------------------------------
// Helpers y syscalls de U-Mode compartidos por todos los módulos
// ---------------------------------------------------------------------------

use crate::task::IpcMessage;

// Códigos de comando globales (Nameserver)
pub const NS_CMD_REGISTER: u32 = 1001;
pub const NS_CMD_LOOKUP:   u32 = 1002;
pub const NS_RESP_SUCCESS: u32 = 2000;
pub const NS_RESP_ERROR:   u32 = 4000;

// Códigos de comando (Input Driver)
pub const INPUT_CMD_GET_KEY: u32 = 2001;
pub const INPUT_RESP_KEY:     u32 = 2002;
pub const INPUT_RESP_EMPTY:   u32 = 2003;

// Códigos de comando (Window Manager)
pub const WM_CMD_CREATE_WINDOW: u32 = 3001;
pub const WM_CMD_DRAW_RECT:     u32 = 3002;
pub const WM_CMD_DRAW_TEXT:     u32 = 3003;
pub const WM_CMD_UPDATE:        u32 = 3004;
pub const WM_RESP_SUCCESS:      u32 = 2000;
pub const WM_RESP_ERROR:        u32 = 4000;

// Códigos de comando (VFS)
pub const VFS_CMD_OPEN:  u32 = 100;
pub const VFS_CMD_READ:  u32 = 101;
pub const VFS_CMD_CLOSE: u32 = 102;
pub const VFS_CMD_LIST:  u32 = 103;
pub const VFS_RESP_OK:   u32 = 200;
pub const VFS_RESP_ERR:  u32 = 400;

// ---------------------------------------------------------------------------
// Syscall wrappers
// ---------------------------------------------------------------------------

pub fn sys_write_fd(fd: usize, s: &str) {
    let ptr = s.as_ptr() as usize;
    let len = s.len();
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 3usize,
            in("a0") fd,
            in("a1") ptr,
            in("a2") len,
            clobber_abi("C"),
        );
    }
}

#[inline(always)]
pub fn user_print(s: &str) { sys_write_fd(1, s); }
#[allow(dead_code)]
#[inline(always)]
pub fn log_error(s: &str) { sys_write_fd(2, s); }
#[allow(dead_code)]
#[inline(always)]
pub fn log_info(s: &str)  { sys_write_fd(3, s); }
#[allow(dead_code)]
#[inline(always)]
pub fn log_debug(s: &str) { sys_write_fd(4, s); }

pub fn user_ipc_send(dest: usize, msg: &IpcMessage) -> isize {
    let mut res: isize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 4usize,
            inout("a0") dest as isize => res,
            in("a1") msg as *const _,
            clobber_abi("C"),
        );
    }
    res
}

pub fn user_ipc_recv(src: usize, msg: &mut IpcMessage) -> isize {
    let mut res: isize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 5usize,
            inout("a0") src as isize => res,
            in("a1") msg as *mut _,
            clobber_abi("C"),
        );
    }
    res
}

#[inline(never)]
pub fn user_yield() {
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 10usize,
            clobber_abi("C"),
        );
    }
}

pub fn str_to_u8_16(s: &str) -> [u8; 16] {
    let mut arr = [0u8; 16];
    let bytes = s.as_bytes();
    let len = core::cmp::min(bytes.len(), 16);
    arr[0..len].copy_from_slice(&bytes[0..len]);
    arr
}

/// Helper NS lookup → devuelve task_id si OK
pub fn ns_lookup(service: &str) -> Option<usize> {
    let mut msg = IpcMessage {
        sender: 0,
        msg_type: NS_CMD_LOOKUP,
        length: 16,
        reserved: 0,
        payload: [0; 32],
    };
    msg.payload[0..16].copy_from_slice(&str_to_u8_16(service));
    if user_ipc_send(3, &msg) == 0 {
        let mut reply = IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
        if user_ipc_recv(3, &mut reply) == 0 && reply.msg_type == NS_RESP_SUCCESS {
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&reply.payload[16..20]);
            return Some(u32::from_ne_bytes(bytes) as usize);
        }
    }
    None
}

/// Helper NS register
pub fn ns_register(service: &str) -> bool {
    let mut msg = IpcMessage {
        sender: 0,
        msg_type: NS_CMD_REGISTER,
        length: 16,
        reserved: 0,
        payload: [0; 32],
    };
    msg.payload[0..16].copy_from_slice(&str_to_u8_16(service));
    if user_ipc_send(3, &msg) == 0 {
        let mut reply = IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
        if user_ipc_recv(3, &mut reply) == 0 {
            return reply.msg_type == NS_RESP_SUCCESS;
        }
    }
    false
}

pub fn make_vfs_open_msg(path: &[u8]) -> IpcMessage {
    let mut msg = IpcMessage {
        sender: 0,
        msg_type: VFS_CMD_OPEN,
        length: path.len() as u32,
        reserved: 0,
        payload: [0; 32],
    };
    let copy_len = path.len().min(32);
    msg.payload[..copy_len].copy_from_slice(&path[..copy_len]);
    msg
}

pub fn make_vfs_read_resp(data: &[u8]) -> IpcMessage {
    let copy_len = data.len().min(32);
    let mut msg = IpcMessage {
        sender: 0,
        msg_type: VFS_RESP_OK,
        length: copy_len as u32,
        reserved: 0,
        payload: [0; 32],
    };
    msg.payload[..copy_len].copy_from_slice(&data[..copy_len]);
    msg
}
