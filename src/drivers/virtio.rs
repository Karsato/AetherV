#![allow(dead_code)]
// Controlador e interfaz común de VirtIO MMIO y Virtqueues

// Estatus de los dispositivos VirtIO (Status Register)
pub const VIRTIO_STATUS_ACKNOWLEDGE: u32 = 1;
pub const VIRTIO_STATUS_DRIVER: u32 = 2;
pub const VIRTIO_STATUS_DRIVER_OK: u32 = 4;
pub const VIRTIO_STATUS_FEATURES_OK: u32 = 8;

// Flags de descriptores de Virtqueue
pub const VIRTQ_DESC_F_NEXT: u16 = 1;     // Indica que hay otro descriptor encadenado
pub const VIRTQ_DESC_F_WRITE: u16 = 2;    // Escritura para el dispositivo (0 = sólo lectura)

#[repr(C)]
pub struct VirtioMmioregs {
    pub magic: u32,             // 0x00: "virt" (0x74726976)
    pub version: u32,           // 0x04: Versión (1 = Legacy, 2 = Modern)
    pub device_id: u32,         // 0x08: ID del dispositivo (16 = GPU, 18 = Input)
    pub vendor_id: u32,         // 0x0c: ID del fabricante (0x554d4551 para QEMU)
    pub device_features: u32,   // 0x10: Características del dispositivo
    pub device_features_sel: u32, // 0x14: Selección de página de características del dispositivo
    pub rsvd0: [u32; 2],        // 0x18 - 0x1c
    pub driver_features: u32,   // 0x20: Características del driver
    pub driver_features_sel: u32, // 0x24: Selección de características del driver
    pub guest_page_size: u32,   // 0x28: Tamaño de página del Guest (sólo Legacy)
    pub rsvd1: u32,             // 0x2c
    pub queue_sel: u32,         // 0x30: Selección de cola
    pub queue_num_max: u32,     // 0x34: Tamaño máximo de cola
    pub queue_num: u32,         // 0x38: Tamaño de cola configurado
    pub queue_align: u32,       // 0x3c: Alineación de cola (sólo Legacy)
    pub queue_pfn: u32,         // 0x40: Dirección física PPN de la cola (sólo Legacy)
    pub queue_ready: u32,       // 0x44: Cola lista (sólo Modern)
    pub rsvd2: [u32; 2],        // 0x48 - 0x4c
    pub queue_notify: u32,      // 0x50: Notificar al dispositivo sobre nuevos comandos
    pub rsvd3: [u32; 3],        // 0x54 - 0x5c
    pub interrupt_status: u32,  // 0x60: Estatus de interrupción
    pub interrupt_ack: u32,     // 0x64: Acknowledge de interrupción
    pub rsvd4: [u32; 2],        // 0x68 - 0x6c
    pub status: u32,            // 0x70: Estatus del dispositivo (R/W)
    pub rsvd5: [u32; 3],        // 0x74 - 0x7c
    pub queue_desc_lo: u32,     // 0x80
    pub queue_desc_hi: u32,     // 0x84
    pub rsvd6: [u32; 2],        // 0x88 - 0x8c
    pub queue_avail_lo: u32,    // 0x90
    pub queue_avail_hi: u32,    // 0x94
    pub rsvd7: [u32; 2],        // 0x98 - 0x9c
    pub queue_used_lo: u32,     // 0xa0
    pub queue_used_hi: u32,     // 0xa4
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct VirtqDesc {
    pub addr: u64,
    pub len: u32,
    pub flags: u16,
    pub next: u16,
}

impl VirtqDesc {
    pub const fn new() -> Self {
        Self {
            addr: 0,
            len: 0,
            flags: 0,
            next: 0,
        }
    }
}

#[repr(C)]
pub struct VirtqAvail {
    pub flags: u16,
    pub idx: u16,
    pub ring: [u16; 32],
}

impl VirtqAvail {
    pub const fn new() -> Self {
        Self {
            flags: 0,
            idx: 0,
            ring: [0; 32],
        }
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct VirtqUsedElem {
    pub id: u32,
    pub len: u32,
}

impl VirtqUsedElem {
    pub const fn new() -> Self {
        Self { id: 0, len: 0 }
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct VirtqUsed {
    pub flags: u16,
    pub idx: u16,
    pub ring: [VirtqUsedElem; 32],
}

impl VirtqUsed {
    pub const fn new() -> Self {
        Self {
            flags: 0,
            idx: 0,
            ring: [VirtqUsedElem::new(); 32],
        }
    }
}

// Layout completo alineado para Virtqueues Legacy (32 descriptores)
#[repr(align(4096))]
#[repr(C)]
pub struct VirtqueueLayout {
    pub descriptors: [VirtqDesc; 32],     // offset 0 (512 bytes)
    pub avail: VirtqAvail,                 // offset 512 (68 bytes)
    pub padding: [u8; 3516],               // Relleno hasta 4KB (4096 bytes)
    pub used: VirtqUsed,                   // offset 4096 (260 bytes)
    pub padding2: [u8; 3836],              // Relleno hasta 8KB (8192 bytes)
}

impl VirtqueueLayout {
    pub const fn new() -> Self {
        Self {
            descriptors: [VirtqDesc::new(); 32],
            avail: VirtqAvail::new(),
            padding: [0; 3516],
            used: VirtqUsed::new(),
            padding2: [0; 3836],
        }
    }
}

// Escanea los slots MMIO de VirtIO y devuelve la base del dispositivo si coincide con el ID
pub fn find_device(device_id: u32) -> Option<usize> {
    for slot in 0..8 {
        let base = 0x10001000 + slot * 0x1000;
        let regs = unsafe { &*(base as *const VirtioMmioregs) };
        if regs.magic == 0x74726976 && regs.device_id == device_id {
            return Some(base);
        }
    }
    None
}
