#![allow(static_mut_refs)]
// Controlador de la GPU virtual de VirtIO (Device ID 16)
fn gpu_print(s: &str) {
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 3,
            inout("a0") s.as_ptr() as usize => _,
            in("a1") s.len(),
            clobber_abi("C"),
        );
    }
}

fn gpu_print_hex(val: usize) {
    let mut buf = [0u8; 18];
    buf[0] = b'0';
    buf[1] = b'x';
    let mut temp = val;
    for i in (2..18).rev() {
        let nibble = (temp & 0xF) as u8;
        buf[i] = match nibble {
            0..=9 => b'0' + nibble,
            10..=15 => b'a' + (nibble - 10),
            _ => unreachable!(),
        };
        temp >>= 4;
    }
    if let Ok(s) = core::str::from_utf8(&buf) {
        gpu_print(s);
    }
}
use crate::drivers::virtio::{
    VirtqueueLayout, find_device,
    VIRTIO_STATUS_ACKNOWLEDGE, VIRTIO_STATUS_DRIVER,
    VIRTIO_STATUS_DRIVER_OK, VIRTQ_DESC_F_NEXT, VIRTQ_DESC_F_WRITE
};

// Comandos de control de la GPU VirtIO
pub const VIRTIO_GPU_CMD_RESOURCE_CREATE_2D: u32 = 0x0101;
pub const VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING: u32 = 0x0106;
pub const VIRTIO_GPU_CMD_SET_SCANOUT: u32 = 0x0103;
pub const VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D: u32 = 0x0105;
pub const VIRTIO_GPU_CMD_RESOURCE_FLUSH: u32 = 0x0104;

// Cabecera de comando de control GPU
#[repr(C)]
#[derive(Clone, Copy)]
pub struct VirtioGpuCtrlHdr {
    pub type_: u32,
    pub flags: u32,
    pub fence_id: u64,
    pub ctx_id: u32,
    pub padding: u32,
}

// 1. Comando: Crear recurso 2D
#[repr(C)]
pub struct VirtioGpuResourceCreate2d {
    pub hdr: VirtioGpuCtrlHdr,
    pub resource_id: u32,
    pub format: u32, // 1 = B8G8R8A8_UNORM
    pub width: u32,
    pub height: u32,
}

// 2. Comando: Vincular memoria RAM física
#[repr(C)]
#[derive(Clone, Copy)]
pub struct VirtioGpuMemEntry {
    pub addr: u64,
    pub len: u32,
    pub padding: u32,
}

#[repr(C)]
pub struct VirtioGpuResourceAttachBacking {
    pub hdr: VirtioGpuCtrlHdr,
    pub resource_id: u32,
    pub nr_entries: u32,
    pub entries: [VirtioGpuMemEntry; 300],
}

// 3. Comando: Establecer pantalla de salida
#[repr(C)]
#[derive(Clone, Copy)]
pub struct VirtioGpuRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[repr(C)]
pub struct VirtioGpuSetScanout {
    pub hdr: VirtioGpuCtrlHdr,
    pub r: VirtioGpuRect,
    pub scanout_id: u32,
    pub resource_id: u32,
}

// 4. Comando: Transferir a Host 2D
#[repr(C)]
pub struct VirtioGpuTransferToHost2d {
    pub hdr: VirtioGpuCtrlHdr,
    pub r: VirtioGpuRect,
    pub offset: u64,
    pub resource_id: u32,
    pub padding: u32,
}

// 5. Comando: Refresco de pantalla
#[repr(C)]
pub struct VirtioGpuResourceFlush {
    pub hdr: VirtioGpuCtrlHdr,
    pub r: VirtioGpuRect,
    pub resource_id: u32,
    pub padding: u32,
}

// Estructura del Framebuffer (640x480 píxeles, 32 bits por píxel: 1.2 MB)
#[repr(align(4096))]
pub struct Framebuffer {
    pub pixels: [u32; 640 * 480],
}

pub static mut FRAMEBUFFER: Framebuffer = Framebuffer {
    pixels: [0; 640 * 480],
};

// Cola de control estática para GPU
pub static mut GPU_CONTROL_QUEUE: VirtqueueLayout = VirtqueueLayout::new();

// Canal de transmisión estático para evitar asignaciones dinámicas
#[repr(C)]
#[repr(align(16))]
pub struct GpuCommandChannel {
    pub req_create: VirtioGpuResourceCreate2d,
    pub req_attach: VirtioGpuResourceAttachBacking,
    pub req_scanout: VirtioGpuSetScanout,
    pub req_transfer: VirtioGpuTransferToHost2d,
    pub req_flush: VirtioGpuResourceFlush,
    pub resp: VirtioGpuCtrlHdr,
}

pub static mut GPU_CHAN: GpuCommandChannel = GpuCommandChannel {
    req_create: VirtioGpuResourceCreate2d {
        hdr: VirtioGpuCtrlHdr { type_: 0, flags: 0, fence_id: 0, ctx_id: 0, padding: 0 },
        resource_id: 0,
        format: 0,
        width: 0,
        height: 0,
    },
    req_attach: VirtioGpuResourceAttachBacking {
        hdr: VirtioGpuCtrlHdr { type_: 0, flags: 0, fence_id: 0, ctx_id: 0, padding: 0 },
        resource_id: 0,
        nr_entries: 0,
        entries: [VirtioGpuMemEntry { addr: 0, len: 0, padding: 0 }; 300],
    },
    req_scanout: VirtioGpuSetScanout {
        hdr: VirtioGpuCtrlHdr { type_: 0, flags: 0, fence_id: 0, ctx_id: 0, padding: 0 },
        r: VirtioGpuRect { x: 0, y: 0, width: 0, height: 0 },
        scanout_id: 0,
        resource_id: 0,
    },
    req_transfer: VirtioGpuTransferToHost2d {
        hdr: VirtioGpuCtrlHdr { type_: 0, flags: 0, fence_id: 0, ctx_id: 0, padding: 0 },
        r: VirtioGpuRect { x: 0, y: 0, width: 0, height: 0 },
        offset: 0,
        resource_id: 0,
        padding: 0,
    },
    req_flush: VirtioGpuResourceFlush {
        hdr: VirtioGpuCtrlHdr { type_: 0, flags: 0, fence_id: 0, ctx_id: 0, padding: 0 },
        r: VirtioGpuRect { x: 0, y: 0, width: 0, height: 0 },
        resource_id: 0,
        padding: 0,
    },
    resp: VirtioGpuCtrlHdr { type_: 0, flags: 0, fence_id: 0, ctx_id: 0, padding: 0 },
};

// Auxiliares de lectura y escritura volátiles de registros MMIO
unsafe fn write_reg(base: usize, offset: usize, val: u32) {
    core::ptr::write_volatile((base + offset) as *mut u32, val);
}

unsafe fn read_reg(base: usize, offset: usize) -> u32 {
    core::ptr::read_volatile((base + offset) as *const u32)
}

fn to_physical(addr: usize) -> usize {
    if addr < 0x80000000 {
        addr + 0x40000000
    } else {
        addr
    }
}

// Envía un comando síncrono al dispositivo GPU rellenando la Virtqueue y esperando respuesta
unsafe fn send_command(
    base: usize,
    req_pa: usize,
    req_len: usize,
    resp_pa: usize,
    resp_len: usize,
) {
    let queue = &mut GPU_CONTROL_QUEUE;

    // Descriptor 0: Datos de petición (Lectura para el dispositivo)
    queue.descriptors[0].addr = to_physical(req_pa) as u64;
    queue.descriptors[0].len = req_len as u32;
    queue.descriptors[0].flags = VIRTQ_DESC_F_NEXT;
    queue.descriptors[0].next = 1;

    // Descriptor 1: Datos de respuesta (Escritura para el dispositivo)
    queue.descriptors[1].addr = to_physical(resp_pa) as u64;
    queue.descriptors[1].len = resp_len as u32;
    queue.descriptors[1].flags = VIRTQ_DESC_F_WRITE;
    queue.descriptors[1].next = 0;

    // Colocar el descriptor de cabeza (0) en el anillo de disponibles
    let avail_idx = queue.avail.idx as usize % 32;
    queue.avail.ring[avail_idx] = 0;

    // Barrera física RISC-V para asegurar que las escrituras del descriptor son visibles en RAM
    core::arch::asm!("fence", options(nostack, preserves_flags));

    queue.avail.idx = queue.avail.idx.wrapping_add(1);

    core::arch::asm!("fence", options(nostack, preserves_flags));

    // Notificar al dispositivo escribiendo el índice de cola (0) en el registro QueueNotify (0x50)
    write_reg(base, 0x50, 0);

    // Esperar respuesta (Sondeo/Polling activo del Used Ring index con lectura volátil)
    let target_idx = queue.avail.idx;
    unsafe {
        while core::ptr::read_volatile(&queue.used.idx as *const u16) != target_idx {
            core::hint::spin_loop();
        }
    }
}

pub static mut GPU_ACTIVE: bool = false;

// Inicialización del subsistema gráfico
pub fn init() {
    let base = match find_device(16) { // ID 16 = GPU
        Some(b) => b,
        None => {
            gpu_print("[GPU] Dispositivo VirtIO GPU no encontrado en ranuras MMIO.\n");
            unsafe { GPU_ACTIVE = false; }
            return;
        }
    };

    gpu_print("[GPU] Inicializando dispositivo gráfico en base MMIO: ");
    gpu_print_hex(base);
    gpu_print("\n");

    unsafe {
        let version = read_reg(base, 0x04);
        gpu_print("[GPU] Versión del hardware VirtIO detectada: ");
        gpu_print_hex(version as usize);
        gpu_print("\n");

        // 1. Reiniciar
        write_reg(base, 0x70, 0); // status = 0

        // 2. Aceptar dispositivo
        let mut status = read_reg(base, 0x70);
        status |= VIRTIO_STATUS_ACKNOWLEDGE;
        write_reg(base, 0x70, status);

        // 3. Establecer driver activo
        status |= VIRTIO_STATUS_DRIVER;
        write_reg(base, 0x70, status);

        // Negociar FEATURES_OK si es Modern (versión 2)
        if version == 2 {
            write_reg(base, 0x20, 0); // driver_features = 0
            status |= 8; // FEATURES_OK
            write_reg(base, 0x70, status);
            let check_status = read_reg(base, 0x70);
            if (check_status & 8) == 0 {
                gpu_print("[GPU ERROR] FEATURES_OK no soportado por el dispositivo!\n");
                return;
            }
        }

        // 4. Configurar Virtqueue 0 (Cola de control de comandos)
        write_reg(base, 0x30, 0); // queue_sel = 0
        let max_num = read_reg(base, 0x34); // queue_num_max
        if max_num < 32 {
            gpu_print("[GPU ERROR] queue_num_max de GPU es menor a 32!\n");
            return;
        }
        write_reg(base, 0x38, 32); // queue_num = 32

        let queue_pa = &raw const GPU_CONTROL_QUEUE as *const VirtqueueLayout as usize;
        let desc_pa = to_physical(queue_pa);

        if version == 2 {
            let desc_pa = to_physical(queue_pa);
            let avail_pa = to_physical(queue_pa + 512);
            let used_pa = to_physical(queue_pa + 4096);

            write_reg(base, 0x80, desc_pa as u32); // queue_desc_lo
            write_reg(base, 0x84, (desc_pa >> 32) as u32); // queue_desc_hi
            write_reg(base, 0x90, avail_pa as u32); // queue_avail_lo
            write_reg(base, 0x94, (avail_pa >> 32) as u32); // queue_avail_hi
            write_reg(base, 0xa0, used_pa as u32); // queue_used_lo
            write_reg(base, 0xa4, (used_pa >> 32) as u32); // queue_used_hi

            write_reg(base, 0x44, 1); // queue_ready = 1
        } else {
            write_reg(base, 0x28, 4096); // guest_page_size = 4096
            write_reg(base, 0x3c, 4096); // queue_align = 4096
            let pfn = desc_pa >> 12;
            write_reg(base, 0x40, pfn as u32); // queue_pfn
        }

        // 5. Activar E/S del dispositivo
        status |= VIRTIO_STATUS_DRIVER_OK;
        write_reg(base, 0x70, status);
        gpu_print("[GPU] Handshake de inicialización VirtIO exitoso.\n");

        // 6. Crear recurso 2D en QEMU
        let chan = &mut GPU_CHAN;
        chan.req_create.hdr.type_ = VIRTIO_GPU_CMD_RESOURCE_CREATE_2D;
        chan.req_create.resource_id = 1;
        chan.req_create.format = 1; // B8G8R8A8_UNORM
        chan.req_create.width = 640;
        chan.req_create.height = 480;

        chan.resp.type_ = 0; // Reset response type
        send_command(
            base,
            &chan.req_create as *const _ as usize,
            core::mem::size_of::<VirtioGpuResourceCreate2d>(),
            &chan.resp as *const _ as usize,
            core::mem::size_of::<VirtioGpuCtrlHdr>(),
        );

        if chan.resp.type_ != 0x1100 {
            gpu_print("[GPU ERROR] No se pudo crear el recurso 2D. Respuesta: ");
            gpu_print_hex(chan.resp.type_ as usize);
            gpu_print("\n");
            return;
        }
        gpu_print("[GPU] Recurso gráfico 2D configurado con resolución 640x480.\n");

        // 7. Enlazar Framebuffer de RAM física (Mapeo página por página para compatibilidad con QEMU)
        chan.req_attach.hdr.type_ = VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING;
        chan.req_attach.resource_id = 1;
        chan.req_attach.nr_entries = 300;
        let fb_addr = to_physical(&raw const FRAMEBUFFER as *const Framebuffer as usize) as u64;
        for i in 0..300 {
            chan.req_attach.entries[i].addr = fb_addr + (i * 4096) as u64;
            chan.req_attach.entries[i].len = 4096;
            chan.req_attach.entries[i].padding = 0;
        }

        chan.resp.type_ = 0; // Reset response type
        send_command(
            base,
            &chan.req_attach as *const _ as usize,
            core::mem::size_of::<VirtioGpuResourceAttachBacking>(),
            &chan.resp as *const _ as usize,
            core::mem::size_of::<VirtioGpuCtrlHdr>(),
        );

        if chan.resp.type_ != 0x1100 {
            gpu_print("[GPU ERROR] No se pudo enlazar el Framebuffer local. Respuesta: ");
            gpu_print_hex(chan.resp.type_ as usize);
            gpu_print("\n");
            return;
        }
        gpu_print("[GPU] Framebuffer local enlazado a la GPU de QEMU.\n");

        // 8. Establecer pantalla principal (Scanout)
        chan.req_scanout.hdr.type_ = VIRTIO_GPU_CMD_SET_SCANOUT;
        chan.req_scanout.scanout_id = 0;
        chan.req_scanout.resource_id = 1;
        chan.req_scanout.r.x = 0;
        chan.req_scanout.r.y = 0;
        chan.req_scanout.r.width = 640;
        chan.req_scanout.r.height = 480;

        chan.resp.type_ = 0; // Reset response type
        send_command(
            base,
            &chan.req_scanout as *const _ as usize,
            core::mem::size_of::<VirtioGpuSetScanout>(),
            &chan.resp as *const _ as usize,
            core::mem::size_of::<VirtioGpuCtrlHdr>(),
        );

        if chan.resp.type_ != 0x1100 {
            gpu_print("[GPU ERROR] No se pudo establecer Scanout. Respuesta: ");
            gpu_print_hex(chan.resp.type_ as usize);
            gpu_print("\n");
            return;
        }
        gpu_print("[GPU] Pantalla de salida asociada al recurso 1.\n");

        // 9. Dibujar patrón degradado e iniciar transferencia
        draw_pattern();
        flush_screen(base);
        gpu_print("[GPU] Patrón gráfico renderizado en pantalla.\n");
        GPU_ACTIVE = true;
    }
}

// Dibuja un degradado cromático en la memoria del Framebuffer
pub fn draw_pattern() {
    unsafe {
        let fb = &mut FRAMEBUFFER;
        for y in 0..480 {
            for x in 0..640 {
                let r = (x * 255 / 640) as u32;
                let g = (y * 255 / 480) as u32;
                let b = ((x + y) * 255 / (640 + 480)) as u32;
                fb.pixels[y * 640 + x] = 0xFF000000 | (r << 16) | (g << 8) | b;
            }
        }
    }
}

pub fn draw_rect(x: usize, y: usize, w: usize, h: usize, color: u32) {
    unsafe {
        let fb = &mut FRAMEBUFFER;
        for dy in 0..h {
            let py = y + dy;
            if py >= 480 { break; }
            for dx in 0..w {
                let px = x + dx;
                if px >= 640 { break; }
                fb.pixels[py * 640 + px] = color;
            }
        }
    }
}

pub const FONT_8X8: [[u8; 8]; 95] = [
    // 32: Space
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    // 33: !
    [0x18, 0x18, 0x18, 0x18, 0x00, 0x00, 0x18, 0x00],
    // 34: "
    [0x36, 0x36, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    // 35: #
    [0x36, 0x36, 0x7f, 0x36, 0x7f, 0x36, 0x36, 0x00],
    // 36: $
    [0x18, 0x3e, 0x60, 0x3c, 0x06, 0x7c, 0x18, 0x00],
    // 37: %
    [0x00, 0x66, 0x66, 0x0c, 0x18, 0x33, 0x33, 0x00],
    // 38: &
    [0x1c, 0x36, 0x1c, 0x3a, 0x66, 0x3f, 0x00, 0x00],
    // 39: '
    [0x18, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    // 40: (
    [0x0c, 0x18, 0x30, 0x30, 0x30, 0x18, 0x0c, 0x00],
    // 41: )
    [0x30, 0x18, 0x0c, 0x0c, 0x0c, 0x18, 0x30, 0x00],
    // 42: *
    [0x00, 0x66, 0x3c, 0xff, 0x3c, 0x66, 0x00, 0x00],
    // 43: +
    [0x00, 0x18, 0x18, 0x7e, 0x18, 0x18, 0x00, 0x00],
    // 44: ,
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x30],
    // 45: -
    [0x00, 0x00, 0x00, 0x7e, 0x00, 0x00, 0x00, 0x00],
    // 46: .
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00],
    // 47: /
    [0x02, 0x04, 0x08, 0x18, 0x30, 0x40, 0x80, 0x00],
    // 48: 0
    [0x3c, 0x66, 0x6e, 0x76, 0x66, 0x66, 0x3c, 0x00],
    // 49: 1
    [0x18, 0x38, 0x18, 0x18, 0x18, 0x18, 0x7e, 0x00],
    // 50: 2
    [0x3c, 0x66, 0x06, 0x1e, 0x30, 0x60, 0x7f, 0x00],
    // 51: 3
    [0x3c, 0x66, 0x06, 0x1c, 0x06, 0x66, 0x3c, 0x00],
    // 52: 4
    [0x06, 0x0e, 0x1e, 0x36, 0x7f, 0x06, 0x06, 0x00],
    // 53: 5
    [0x7f, 0x60, 0x7c, 0x06, 0x06, 0x66, 0x3c, 0x00],
    // 54: 6
    [0x3c, 0x60, 0x7c, 0x66, 0x66, 0x66, 0x3c, 0x00],
    // 55: 7
    [0x7f, 0x06, 0x0c, 0x18, 0x30, 0x30, 0x30, 0x00],
    // 56: 8
    [0x3c, 0x66, 0x66, 0x3c, 0x66, 0x66, 0x3c, 0x00],
    // 57: 9
    [0x3c, 0x66, 0x66, 0x3e, 0x06, 0x0c, 0x38, 0x00],
    // 58: :
    [0x00, 0x18, 0x18, 0x00, 0x18, 0x18, 0x00, 0x00],
    // 59: ;
    [0x00, 0x18, 0x18, 0x00, 0x18, 0x18, 0x30, 0x00],
    // 60: <
    [0x0c, 0x18, 0x30, 0x60, 0x30, 0x18, 0x0c, 0x00],
    // 61: =
    [0x00, 0x00, 0x7e, 0x00, 0x7e, 0x00, 0x00, 0x00],
    // 62: >
    [0x30, 0x18, 0x0c, 0x06, 0x0c, 0x18, 0x30, 0x00],
    // 63: ?
    [0x3c, 0x66, 0x06, 0x0c, 0x18, 0x00, 0x18, 0x00],
    // 64: @
    [0x3e, 0x66, 0x6f, 0x6b, 0x6f, 0x60, 0x3e, 0x00],
    // 65: A
    [0x18, 0x3c, 0x66, 0x66, 0x7e, 0x66, 0x66, 0x00],
    // 66: B
    [0x7c, 0x66, 0x66, 0x7c, 0x66, 0x66, 0x7c, 0x00],
    // 67: C
    [0x3c, 0x66, 0x60, 0x60, 0x60, 0x66, 0x3c, 0x00],
    // 68: D
    [0x78, 0x6c, 0x66, 0x66, 0x66, 0x6c, 0x78, 0x00],
    // 69: E
    [0x7f, 0x60, 0x60, 0x7c, 0x60, 0x60, 0x7f, 0x00],
    // 70: F
    [0x7f, 0x60, 0x60, 0x7c, 0x60, 0x60, 0x60, 0x00],
    // 71: G
    [0x3c, 0x66, 0x60, 0x6e, 0x66, 0x66, 0x3e, 0x00],
    // 72: H
    [0x66, 0x66, 0x66, 0x7e, 0x66, 0x66, 0x66, 0x00],
    // 73: I
    [0x3e, 0x08, 0x08, 0x08, 0x08, 0x08, 0x3e, 0x00],
    // 74: J
    [0x1f, 0x04, 0x04, 0x04, 0x04, 0x24, 0x1c, 0x00],
    // 75: K
    [0x66, 0x6c, 0x78, 0x70, 0x78, 0x6c, 0x66, 0x00],
    // 76: L
    [0x60, 0x60, 0x60, 0x60, 0x60, 0x60, 0x7f, 0x00],
    // 77: M
    [0x63, 0x77, 0x7f, 0x6b, 0x63, 0x63, 0x63, 0x00],
    // 78: N
    [0x66, 0x76, 0x7e, 0x76, 0x66, 0x66, 0x66, 0x00],
    // 79: O
    [0x3c, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3c, 0x00],
    // 80: P
    [0x7c, 0x66, 0x66, 0x7c, 0x60, 0x60, 0x60, 0x00],
    // 81: Q
    [0x3c, 0x66, 0x66, 0x66, 0x6a, 0x6c, 0x3e, 0x00],
    // 82: R
    [0x7c, 0x66, 0x66, 0x7c, 0x6c, 0x66, 0x66, 0x00],
    // 83: S
    [0x3c, 0x66, 0x30, 0x1c, 0x06, 0x66, 0x3c, 0x00],
    // 84: T
    [0x7e, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00],
    // 85: U
    [0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3c, 0x00],
    // 86: V
    [0x66, 0x66, 0x66, 0x66, 0x3c, 0x24, 0x18, 0x00],
    // 87: W
    [0x63, 0x63, 0x63, 0x6b, 0x7f, 0x77, 0x63, 0x00],
    // 88: X
    [0x66, 0x66, 0x3c, 0x18, 0x3c, 0x66, 0x66, 0x00],
    // 89: Y
    [0x66, 0x66, 0x66, 0x3c, 0x18, 0x18, 0x18, 0x00],
    // 90: Z
    [0x7f, 0x06, 0x0c, 0x18, 0x30, 0x60, 0x7f, 0x00],
    // 91: [
    [0x1e, 0x18, 0x18, 0x18, 0x18, 0x18, 0x1e, 0x00],
    // 92: \
    [0x40, 0x30, 0x18, 0x0c, 0x06, 0x03, 0x01, 0x00],
    // 93: ]
    [0x3c, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x3c, 0x00],
    // 94: ^
    [0x08, 0x1c, 0x36, 0x63, 0x00, 0x00, 0x00, 0x00],
    // 95: _
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0x00],
    // 96: `
    [0x18, 0x0c, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00],
    // 97: a
    [0x00, 0x00, 0x3c, 0x06, 0x3e, 0x66, 0x3b, 0x00],
    // 98: b
    [0x60, 0x60, 0x7c, 0x66, 0x66, 0x66, 0x7c, 0x00],
    // 99: c
    [0x00, 0x00, 0x3c, 0x60, 0x60, 0x66, 0x3c, 0x00],
    // 100: d
    [0x06, 0x06, 0x3e, 0x66, 0x66, 0x66, 0x3d, 0x00],
    // 101: e
    [0x00, 0x00, 0x3c, 0x66, 0x7e, 0x60, 0x3c, 0x00],
    // 102: f
    [0x0e, 0x18, 0x3e, 0x18, 0x18, 0x18, 0x18, 0x00],
    // 103: g
    [0x00, 0x00, 0x3d, 0x66, 0x66, 0x3e, 0x06, 0x3c],
    // 104: h
    [0x60, 0x60, 0x7c, 0x66, 0x66, 0x66, 0x66, 0x00],
    // 105: i
    [0x18, 0x00, 0x38, 0x18, 0x18, 0x18, 0x3c, 0x00],
    // 106: j
    [0x06, 0x00, 0x0e, 0x06, 0x06, 0x06, 0x06, 0x3c],
    // 107: k
    [0x60, 0x60, 0x66, 0x6c, 0x78, 0x6c, 0x66, 0x00],
    // 108: l
    [0x38, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3c, 0x00],
    // 109: m
    [0x00, 0x00, 0x7c, 0x6b, 0x6b, 0x6b, 0x6b, 0x00],
    // 110: n
    [0x00, 0x00, 0x7c, 0x66, 0x66, 0x66, 0x66, 0x00],
    // 111: o
    [0x00, 0x00, 0x3c, 0x66, 0x66, 0x66, 0x3c, 0x00],
    // 112: p
    [0x00, 0x00, 0x7c, 0x66, 0x66, 0x7c, 0x60, 0x60],
    // 113: q
    [0x00, 0x00, 0x3e, 0x66, 0x66, 0x3e, 0x06, 0x06],
    // 114: r
    [0x00, 0x00, 0x7c, 0x66, 0x60, 0x60, 0x60, 0x00],
    // 115: s
    [0x00, 0x00, 0x3e, 0x60, 0x3c, 0x06, 0x7c, 0x00],
    // 116: t
    [0x18, 0x18, 0x7e, 0x18, 0x18, 0x18, 0x0e, 0x00],
    // 117: u
    [0x00, 0x00, 0x66, 0x66, 0x66, 0x66, 0x3b, 0x00],
    // 118: v
    [0x00, 0x00, 0x66, 0x66, 0x3c, 0x24, 0x18, 0x00],
    // 119: w
    [0x00, 0x00, 0x63, 0x6b, 0x7f, 0x3e, 0x36, 0x00],
    // 120: x
    [0x00, 0x00, 0x66, 0x3c, 0x18, 0x3c, 0x66, 0x00],
    // 121: y
    [0x00, 0x00, 0x66, 0x66, 0x3e, 0x06, 0x0c, 0x38],
    // 122: z
    [0x00, 0x00, 0x7e, 0x0c, 0x18, 0x30, 0x7e, 0x00],
    // 123: {
    [0x0c, 0x18, 0x18, 0x30, 0x18, 0x18, 0x0c, 0x00],
    // 124: |
    [0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00],
    // 125: }
    [0x30, 0x18, 0x18, 0x0c, 0x18, 0x18, 0x30, 0x00],
    // 126: ~
    [0x76, 0x59, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
];

pub fn draw_char(x: usize, y: usize, c: char, color: u32) {
    let ascii = c as usize;
    if ascii < 32 || ascii > 126 {
        return;
    }
    let font_idx = ascii - 32;
    unsafe {
        let fb = &mut FRAMEBUFFER;
        let char_data = FONT_8X8[font_idx];
        for row in 0..8 {
            let py = y + row;
            if py >= 480 { break; }
            let row_data = char_data[row];
            for col in 0..8 {
                let px = x + col;
                if px >= 640 { break; }
                if (row_data & (0x80 >> col)) != 0 {
                    fb.pixels[py * 640 + px] = color;
                }
            }
        }
    }
}

pub fn draw_string(x: usize, y: usize, s: &str, color: u32) {
    let mut cx = x;
    let mut cy = y;
    for c in s.chars() {
        if c == '\n' {
            cx = x;
            cy += 8;
            if cy >= 480 { break; }
        } else {
            draw_char(cx, cy, c, color);
            cx += 8;
            if cx >= 640 {
                cx = x;
                cy += 8;
                if cy >= 480 { break; }
            }
        }
    }
}

// Sincroniza y vuelca el Framebuffer local a la pantalla de QEMU
pub fn flush_screen(base: usize) {
    unsafe {
        if !GPU_ACTIVE {
            return;
        }
        let chan = &mut GPU_CHAN;

        // A. Copiar datos del Framebuffer al buffer del Host
        chan.req_transfer.hdr.type_ = VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D;
        chan.req_transfer.resource_id = 1;
        chan.req_transfer.offset = 0;
        chan.req_transfer.r.x = 0;
        chan.req_transfer.r.y = 0;
        chan.req_transfer.r.width = 640;
        chan.req_transfer.r.height = 480;

        send_command(
            base,
            &chan.req_transfer as *const _ as usize,
            core::mem::size_of::<VirtioGpuTransferToHost2d>(),
            &chan.resp as *const _ as usize,
            core::mem::size_of::<VirtioGpuCtrlHdr>(),
        );

        // B. Ordenar a la pantalla virtual redibujar el área
        chan.req_flush.hdr.type_ = VIRTIO_GPU_CMD_RESOURCE_FLUSH;
        chan.req_flush.resource_id = 1;
        chan.req_flush.r.x = 0;
        chan.req_flush.r.y = 0;
        chan.req_flush.r.width = 640;
        chan.req_flush.r.height = 480;

        send_command(
            base,
            &chan.req_flush as *const _ as usize,
            core::mem::size_of::<VirtioGpuResourceFlush>(),
            &chan.resp as *const _ as usize,
            core::mem::size_of::<VirtioGpuCtrlHdr>(),
        );
    }
}
