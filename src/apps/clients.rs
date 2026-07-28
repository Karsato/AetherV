// src/apps/clients.rs

use super::{user_ipc_recv, user_ipc_send, user_print, log_debug, user_yield,
            ns_lookup,
            NS_CMD_LOOKUP, NS_RESP_SUCCESS,
            WM_CMD_CREATE_WINDOW, WM_CMD_DRAW_RECT, WM_CMD_DRAW_TEXT, WM_CMD_UPDATE,
            WM_RESP_SUCCESS};
use crate::task::IpcMessage;

fn u32_to_str(val: u32, buf: &mut [u8]) -> usize {
    if val == 0 { buf[0] = b'0'; return 1; }
    let mut temp = val;
    let mut len = 0;
    while temp > 0 { len += 1; temp /= 10; }
    let mut temp = val;
    for i in (0..len).rev() {
        buf[i] = b'0' + (temp % 10) as u8;
        temp /= 10;
    }
    len
}

pub fn window_client_1() {
    user_print("[Client 1] Buscando 'wm' en el Nameserver...\n");
    let wm_task_id = loop {
        if let Some(id) = ns_lookup("wm") { break id; }
        user_yield();
    };
    log_debug("[Client 1] Conectado al Window Manager!\n");

    let mut create_msg = IpcMessage {
        sender: 0,
        msg_type: WM_CMD_CREATE_WINDOW,
        length: 24,
        reserved: 0,
        payload: [0; 32],
    };
    create_msg.payload[0..16].copy_from_slice(b"Bouncing Ball\0\0\0");
    create_msg.payload[16] = 30; create_msg.payload[17] = 240;
    create_msg.payload[18] = 135; create_msg.payload[19] = 95;
    create_msg.payload[20] = 30; create_msg.payload[21] = 30; create_msg.payload[22] = 46;

    let mut reply = IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
    let mut win_id = 0;
    if user_ipc_send(wm_task_id, &create_msg) == 0 {
        if user_ipc_recv(wm_task_id, &mut reply) == 0 && reply.msg_type == WM_RESP_SUCCESS {
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&reply.payload[0..4]);
            win_id = u32::from_ne_bytes(bytes) as usize;
        }
    }

    let mut ball_x: i32 = 20;
    let mut ball_y: i32 = 30;
    let mut ball_dx: i32 = 6;
    let mut ball_dy: i32 = 4;
    let max_w: i32 = 266 - 8;
    let max_h: i32 = 168 - 8;

    loop {
        ball_x += ball_dx; ball_y += ball_dy;
        if ball_x <= 4 || ball_x >= max_w { ball_dx = -ball_dx; }
        if ball_y <= 4 || ball_y >= max_h { ball_dy = -ball_dy; }

        let mut clear_msg = IpcMessage { sender: 0, msg_type: WM_CMD_DRAW_RECT, length: 8, reserved: 0, payload: [0; 32] };
        clear_msg.payload[0] = win_id as u8;
        clear_msg.payload[1] = 0; clear_msg.payload[2] = 0;
        clear_msg.payload[3] = 255; clear_msg.payload[4] = 168;
        clear_msg.payload[5] = 30; clear_msg.payload[6] = 30; clear_msg.payload[7] = 46;
        user_ipc_send(wm_task_id, &clear_msg);
        user_ipc_recv(wm_task_id, &mut reply);

        let mut text_msg = IpcMessage { sender: 0, msg_type: WM_CMD_DRAW_TEXT, length: 26, reserved: 0, payload: [0; 32] };
        text_msg.payload[0] = win_id as u8; text_msg.payload[1] = 10; text_msg.payload[2] = 10;
        text_msg.payload[3] = 255; text_msg.payload[4] = 255; text_msg.payload[5] = 255;
        text_msg.payload[6..26].copy_from_slice(b"Bouncing Ball Demo.\0");
        user_ipc_send(wm_task_id, &text_msg);
        user_ipc_recv(wm_task_id, &mut reply);

        let mut ball_msg = IpcMessage { sender: 0, msg_type: WM_CMD_DRAW_TEXT, length: 26, reserved: 0, payload: [0; 32] };
        ball_msg.payload[0] = win_id as u8;
        ball_msg.payload[1] = ball_x as u8; ball_msg.payload[2] = ball_y as u8;
        ball_msg.payload[3] = 248; ball_msg.payload[4] = 196; ball_msg.payload[5] = 113;
        ball_msg.payload[6..26].copy_from_slice(b"O\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0");
        user_ipc_send(wm_task_id, &ball_msg);
        user_ipc_recv(wm_task_id, &mut reply);

        let update_msg = IpcMessage { sender: 0, msg_type: WM_CMD_UPDATE, length: 0, reserved: 0, payload: [0; 32] };
        user_ipc_send(wm_task_id, &update_msg);
        user_ipc_recv(wm_task_id, &mut reply);

        for _ in 0..10 { user_yield(); }
    }
}

pub fn window_client_2() {
    user_print("[Client 2] Buscando 'wm' en el Nameserver...\n");
    let wm_task_id = loop {
        if let Some(id) = ns_lookup("wm") { break id; }
        user_yield();
    };
    log_debug("[Client 2] Conectado al Window Manager!\n");

    let mut create_msg = IpcMessage {
        sender: 0,
        msg_type: WM_CMD_CREATE_WINDOW,
        length: 24,
        reserved: 0,
        payload: [0; 32],
    };
    create_msg.payload[0..16].copy_from_slice(b"Performance\0\0\0\0\0");
    create_msg.payload[16] = 165; create_msg.payload[17] = 240;
    create_msg.payload[18] = 140; create_msg.payload[19] = 95;
    create_msg.payload[20] = 26; create_msg.payload[21] = 27; create_msg.payload[22] = 38;

    let mut reply = IpcMessage { sender: 0, msg_type: 0, length: 0, reserved: 0, payload: [0; 32] };
    let mut win_id = 0;
    if user_ipc_send(wm_task_id, &create_msg) == 0 {
        if user_ipc_recv(wm_task_id, &mut reply) == 0 && reply.msg_type == WM_RESP_SUCCESS {
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&reply.payload[0..4]);
            win_id = u32::from_ne_bytes(bytes) as usize;
        }
    }

    let mut counter: u32 = 0;
    loop {
        counter = counter.wrapping_add(1);

        let mut clear_msg = IpcMessage { sender: 0, msg_type: WM_CMD_DRAW_RECT, length: 8, reserved: 0, payload: [0; 32] };
        clear_msg.payload[0] = win_id as u8;
        clear_msg.payload[1] = 0; clear_msg.payload[2] = 0;
        clear_msg.payload[3] = 255; clear_msg.payload[4] = 168;
        clear_msg.payload[5] = 26; clear_msg.payload[6] = 27; clear_msg.payload[7] = 38;
        user_ipc_send(wm_task_id, &clear_msg);
        user_ipc_recv(wm_task_id, &mut reply);

        let mut text_msg = IpcMessage { sender: 0, msg_type: WM_CMD_DRAW_TEXT, length: 26, reserved: 0, payload: [0; 32] };
        text_msg.payload[0] = win_id as u8;
        text_msg.payload[1] = 15; text_msg.payload[2] = 20;
        text_msg.payload[3] = 122; text_msg.payload[4] = 162; text_msg.payload[5] = 247;
        let mut text_buf = [0u8; 20];
        text_buf[0..9].copy_from_slice(b"Counter: ");
        let mut num_buf = [0u8; 10];
        let num_len = u32_to_str(counter, &mut num_buf);
        for i in 0..num_len { text_buf[9 + i] = num_buf[i]; }
        text_msg.payload[6..26].copy_from_slice(&text_buf);
        user_ipc_send(wm_task_id, &text_msg);
        user_ipc_recv(wm_task_id, &mut reply);

        let progress = (counter / 5) % 15;
        let mut prog_buf = [b' '; 20];
        prog_buf[0] = b'[';
        for i in 0..15 {
            if (i as u32) < progress { prog_buf[1 + i] = b'='; }
            else if (i as u32) == progress { prog_buf[1 + i] = b'>'; }
            else { prog_buf[1 + i] = b' '; }
        }
        prog_buf[16] = b']';
        let mut prog_msg = IpcMessage { sender: 0, msg_type: WM_CMD_DRAW_TEXT, length: 26, reserved: 0, payload: [0; 32] };
        prog_msg.payload[0] = win_id as u8;
        prog_msg.payload[1] = 15; prog_msg.payload[2] = 45;
        prog_msg.payload[3] = 187; prog_msg.payload[4] = 154; prog_msg.payload[5] = 247;
        prog_msg.payload[6..26].copy_from_slice(&prog_buf);
        user_ipc_send(wm_task_id, &prog_msg);
        user_ipc_recv(wm_task_id, &mut reply);

        let mut title_msg = IpcMessage { sender: 0, msg_type: WM_CMD_DRAW_TEXT, length: 26, reserved: 0, payload: [0; 32] };
        title_msg.payload[0] = win_id as u8;
        title_msg.payload[1] = 15; title_msg.payload[2] = 80;
        title_msg.payload[3] = 94; title_msg.payload[4] = 211; title_msg.payload[5] = 162;
        title_msg.payload[6..26].copy_from_slice(b"System tick active\0\0");
        user_ipc_send(wm_task_id, &title_msg);
        user_ipc_recv(wm_task_id, &mut reply);

        let update_msg = IpcMessage { sender: 0, msg_type: WM_CMD_UPDATE, length: 0, reserved: 0, payload: [0; 32] };
        user_ipc_send(wm_task_id, &update_msg);
        user_ipc_recv(wm_task_id, &mut reply);

        for _ in 0..10 { user_yield(); }
    }
}

/// PID 8: cliente VFS pasivo (mantiene slot, no disputa entrada con Shell)
pub fn vfs_client() {
    loop { user_yield(); }
}
