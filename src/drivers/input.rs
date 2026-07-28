#![allow(static_mut_refs, dead_code)]
// Controlador de dispositivo de entrada VirtIO Input (teclado/ratón, Device ID 18)

use crate::drivers::virtio::{
    VirtqueueLayout, VIRTIO_STATUS_ACKNOWLEDGE, VIRTIO_STATUS_DRIVER,
    VIRTIO_STATUS_DRIVER_OK, VIRTQ_DESC_F_WRITE,
};

fn input_print(s: &str) {
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 3,
            in("a0") 4, // FD 3 (LOG_INFO)
            in("a1") s.as_ptr() as usize,
            in("a2") s.len(),
            clobber_abi("C"),
        );
    }
}

fn input_print_hex(val: usize) {
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
        input_print(s);
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct VirtioInputEvent {
    pub event_type: u16,
    pub code: u16,
    pub value: u32,
}

// Virtqueue 0: eventq
pub static mut INPUT_EVENT_QUEUE: VirtqueueLayout = VirtqueueLayout::new();

// Buffers para los eventos
pub static mut INPUT_EVENTS: [VirtioInputEvent; 32] = [VirtioInputEvent {
    event_type: 0,
    code: 0,
    value: 0,
}; 32];

pub static mut INPUT_DEVICE_BASE: usize = 0;
pub static mut INPUT_ACTIVE: bool = false;
pub static mut LAST_USED_IDX: u16 = 0;

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

pub fn find_keyboard_device() -> Option<usize> {
    for slot in 0..8 {
        let base = 0x10001000 + slot * 0x1000;
        unsafe {
            let magic = core::ptr::read_volatile(base as *const u32);
            let dev_id = core::ptr::read_volatile((base + 8) as *const u32);
            if magic == 0x74726976 && dev_id == 18 {
                // Seleccionar config de nombre
                core::ptr::write_volatile((base + 0x100) as *mut u8, 1); // select = 1 (VIRTIO_INPUT_CFG_ID_NAME)
                core::ptr::write_volatile((base + 0x101) as *mut u8, 0); // subsel = 0
                let size = core::ptr::read_volatile((base + 0x102) as *const u8) as usize;
                
                let mut name_buf = [0u8; 32];
                let read_size = core::cmp::min(size, 32);
                for i in 0..read_size {
                    name_buf[i] = core::ptr::read_volatile((base + 0x104 + i) as *const u8);
                }
                
                let mut is_keyboard = false;
                for window in name_buf[..read_size].windows(5) {
                    if window == b"Keybo" || window == b"keybo" {
                        is_keyboard = true;
                        break;
                    }
                }
                
                if is_keyboard {
                    return Some(base);
                }
            }
        }
    }
    None
}

// Inicialización del dispositivo VirtIO Input
pub fn init() {
    let base = match find_keyboard_device() {
        Some(b) => b,
        None => {
            input_print("[Input] Dispositivo de teclado VirtIO Keyboard no encontrado en ranuras MMIO.\n");
            unsafe {
                INPUT_ACTIVE = false;
            }
            return;
        }
    };

    input_print("[Input] Inicializando dispositivo de entrada en base MMIO: ");
    input_print_hex(base);
    input_print("\n");

    unsafe {
        let version = read_reg(base, 0x04);
        input_print("[Input] Versión del hardware VirtIO detectada: ");
        input_print_hex(version as usize);
        input_print("\n");

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
                input_print("[Input ERROR] FEATURES_OK no soportado por el dispositivo!\n");
                return;
            }
        }

        // 4. Configurar Virtqueue 0 (eventq)
        write_reg(base, 0x30, 0); // queue_sel = 0
        let max_num = read_reg(base, 0x34); // queue_num_max
        if max_num < 32 {
            input_print("[Input ERROR] queue_num_max de Input es menor a 32!\n");
            return;
        }
        write_reg(base, 0x38, 32); // queue_num = 32

        let queue_pa = &raw const INPUT_EVENT_QUEUE as *const VirtqueueLayout as usize;
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

        // Poblar descriptores para recibir eventos del dispositivo
        for i in 0..32 {
            let event_pa = &raw const INPUT_EVENTS[i] as *const VirtioInputEvent as usize;
            INPUT_EVENT_QUEUE.descriptors[i].addr = to_physical(event_pa) as u64;
            INPUT_EVENT_QUEUE.descriptors[i].len = core::mem::size_of::<VirtioInputEvent>() as u32;
            INPUT_EVENT_QUEUE.descriptors[i].flags = VIRTQ_DESC_F_WRITE;
            INPUT_EVENT_QUEUE.descriptors[i].next = 0;
            INPUT_EVENT_QUEUE.avail.ring[i] = i as u16;
        }

        core::arch::asm!("fence", options(nostack, preserves_flags));
        INPUT_EVENT_QUEUE.avail.idx = 32;
        core::arch::asm!("fence", options(nostack, preserves_flags));

        // 5. Configuración completada con éxito
        status |= VIRTIO_STATUS_DRIVER_OK;
        write_reg(base, 0x70, status);

        // Notificar al dispositivo de que hay buffers disponibles
        write_reg(base, 0x50, 0);

        INPUT_DEVICE_BASE = base;
        INPUT_ACTIVE = true;
        LAST_USED_IDX = 0;

        input_print("[Input] Inicialización del dispositivo de entrada completada.\n");
    }
}

// Procesa eventos de la Virtqueue
pub fn process_events<F: FnMut(VirtioInputEvent)>(mut handler: F) {
    unsafe {
        if !INPUT_ACTIVE {
            return;
        }
        let base = INPUT_DEVICE_BASE;
        let used_idx = core::ptr::read_volatile(&INPUT_EVENT_QUEUE.used.idx as *const u16);
        let mut progress = false;

        while LAST_USED_IDX != used_idx {
            let ring_slot = LAST_USED_IDX as usize % 32;
            let desc_idx = INPUT_EVENT_QUEUE.used.ring[ring_slot].id as usize;

            // Leer evento
            let event = INPUT_EVENTS[desc_idx];

            // Invocar el manejador
            handler(event);

            // Reciclar el descriptor
            let avail_slot = INPUT_EVENT_QUEUE.avail.idx as usize % 32;
            INPUT_EVENT_QUEUE.avail.ring[avail_slot] = desc_idx as u16;

            core::arch::asm!("fence", options(nostack, preserves_flags));
            INPUT_EVENT_QUEUE.avail.idx = INPUT_EVENT_QUEUE.avail.idx.wrapping_add(1);

            LAST_USED_IDX = LAST_USED_IDX.wrapping_add(1);
            progress = true;
        }

        if progress {
            core::arch::asm!("fence", options(nostack, preserves_flags));
            // Acknowledge de la interrupción en el dispositivo VirtIO
            let int_status = read_reg(base, 0x60);
            write_reg(base, 0x64, int_status & 0x3);

            // Notificar al dispositivo
            write_reg(base, 0x50, 0);
        }
    }
}
