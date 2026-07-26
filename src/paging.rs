#![allow(static_mut_refs, dead_code)]
// Memoria Virtual y Paginación Sv39 en RISC-V
use crate::sbi;

// Bits de control de Page Table Entry (PTE)
pub const PTE_V: u8 = 1 << 0; // Valid
pub const PTE_R: u8 = 1 << 1; // Readable
pub const PTE_W: u8 = 1 << 2; // Writable
pub const PTE_X: u8 = 1 << 3; // Executable
pub const PTE_U: u8 = 1 << 4; // User mode accessible
pub const PTE_G: u8 = 1 << 5; // Global mapping
pub const PTE_A: u8 = 1 << 6; // Accessed
pub const PTE_D: u8 = 1 << 7; // Dirty

#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct PTE(u64);

impl PTE {
    pub const fn new() -> Self {
        PTE(0)
    }

    pub fn is_valid(&self) -> bool {
        (self.0 & (PTE_V as u64)) != 0
    }

    pub fn set_valid(&mut self, valid: bool) {
        if valid {
            self.0 |= PTE_V as u64;
        } else {
            self.0 &= !(PTE_V as u64);
        }
    }

    pub fn get_ppn(&self) -> usize {
        ((self.0 >> 10) & 0x3FFFFFFFFFFFF) as usize
    }

    pub fn set_ppn(&mut self, ppn: usize) {
        // Limpiar PPN (bits 53:10)
        self.0 &= !(0x3FFFFFFFFFFFF << 10);
        self.0 |= (ppn as u64 & 0x3FFFFFFFFFFFF) << 10;
    }

    pub fn set_flags(&mut self, flags: u8) {
        // Limpiar los bits de control (bits 7:0)
        self.0 &= !0xFF;
        self.0 |= flags as u64;
    }

    pub fn get_table_ptr(&self) -> *mut PageTable {
        (self.get_ppn() << 12) as *mut PageTable
    }
}

#[repr(align(4096))]
pub struct PageTable {
    pub entries: [PTE; 512],
}

impl PageTable {
    pub const fn new() -> Self {
        PageTable {
            entries: [PTE::new(); 512],
        }
    }
}

// Asignador simple de marcos de páginas físicas (Bump Allocator)
pub struct SimpleFrameAllocator {
    current_addr: usize,
    end_addr: usize,
}

impl SimpleFrameAllocator {
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            current_addr: (start + 4095) & !4095, // Alinear a 4KB
            end_addr: end,
        }
    }

    pub fn alloc(&mut self) -> Option<usize> {
        if self.current_addr + 4096 <= self.end_addr {
            let addr = self.current_addr;
            self.current_addr += 4096;
            Some(addr)
        } else {
            None
        }
    }
}

// Mapea una única página virtual de 4KB a una dirección física con permisos
pub fn map_page(
    root: &mut PageTable,
    va: usize,
    pa: usize,
    flags: u8,
    allocator: &mut SimpleFrameAllocator,
) {
    let vpn = [
        (va >> 12) & 0x1FF, // VPN[0]
        (va >> 21) & 0x1FF, // VPN[1]
        (va >> 30) & 0x1FF, // VPN[2]
    ];

    let mut current_table = root as *mut PageTable;

    // Recorrer los niveles de directorios L2 y L1
    for level in (1..=2).rev() {
        let entry = unsafe { &mut (*current_table).entries[vpn[level]] };
        if !entry.is_valid() {
            // Reservar una página física para la subtabla
            let new_table_pa = allocator.alloc().expect("OOM asignando tabla de páginas");
            // Limpiar la subtabla
            unsafe {
                core::ptr::write_bytes(new_table_pa as *mut u8, 0, 4096);
            }
            entry.set_ppn(new_table_pa >> 12);
            entry.set_valid(true);
        }
        current_table = entry.get_table_ptr();
    }

    // Mapear en la tabla hoja L0
    let entry = unsafe { &mut (*current_table).entries[vpn[0]] };
    entry.set_ppn(pa >> 12);
    // V = 1, A = 1, D = 1 y permisos pasados
    entry.set_flags(flags | PTE_V | PTE_A | PTE_D);
}

// Mapea un rango contiguo de memoria virtual a física (alineado a 4KB)
pub fn map_range(
    root: &mut PageTable,
    start_va: usize,
    start_pa: usize,
    size: usize,
    flags: u8,
    allocator: &mut SimpleFrameAllocator,
) {
    let pages = (size + 4095) / 4096;
    for i in 0..pages {
        map_page(
            root,
            start_va + i * 4096,
            start_pa + i * 4096,
            flags,
            allocator,
        );
    }
}

// Tabla de páginas raíz estática alineada a 4KB
pub static mut KERNEL_PGTABLE: PageTable = PageTable::new();

// Símbolos del linker script
extern "C" {
    static stext: u8;
    static etext: u8;
    static srodata: u8;
    static erodata: u8;
    static sdata: u8;
    static ekernel: u8;
}

pub fn init() {
    // Obtener los límites definidos por el linker script
    let stext_addr = unsafe { &stext as *const u8 as usize };
    let etext_addr = unsafe { &etext as *const u8 as usize };
    let srodata_addr = unsafe { &srodata as *const u8 as usize };
    let erodata_addr = unsafe { &erodata as *const u8 as usize };
    let sdata_addr = unsafe { &sdata as *const u8 as usize };
    let ekernel_addr = unsafe { &ekernel as *const u8 as usize };

    sbi::print_str("[Paging] Limites del Kernel detectados:\n");
    sbi::print_str("  .text   : ");
    sbi::print_hex(stext_addr);
    sbi::print_str(" - ");
    sbi::print_hex(etext_addr);
    sbi::print_str("\n  .rodata : ");
    sbi::print_hex(srodata_addr);
    sbi::print_str(" - ");
    sbi::print_hex(erodata_addr);
    sbi::print_str("\n  .data   : ");
    sbi::print_hex(sdata_addr);
    sbi::print_str(" - ");
    sbi::print_hex(ekernel_addr);
    sbi::print_str("\n");

    // Instanciar el asignador para utilizar desde el fin de kernel hasta el fin de RAM (128 MB)
    let mut allocator = SimpleFrameAllocator::new(ekernel_addr, 0x88000000);

    // Limpiar la tabla raíz antes de usarla
    unsafe {
        core::ptr::write_bytes(&mut KERNEL_PGTABLE as *mut PageTable as *mut u8, 0, 4096);
    }

    // 1. Mapeo UART MMIO (0x1000_0000)
    map_range(
        unsafe { &mut KERNEL_PGTABLE },
        0x1000_0000,
        0x1000_0000,
        4096,
        PTE_R | PTE_W,
        &mut allocator,
    );

    // 2. Mapeo VirtIO MMIO (0x1000_1000 - 0x1000_9000) - R-W-U para permitir drivers en U-mode
    map_range(
        unsafe { &mut KERNEL_PGTABLE },
        0x1000_1000,
        0x1000_1000,
        0x8000,
        PTE_R | PTE_W | PTE_U,
        &mut allocator,
    );

    // 3. Mapeo OpenSBI (0x8000_0000 - 0x8020_0000) - R-X
    map_range(
        unsafe { &mut KERNEL_PGTABLE },
        0x8000_0000,
        0x8000_0000,
        0x200000,
        PTE_R | PTE_X,
        &mut allocator,
    );

    // 4. Mapeo Kernel .text para S-mode (sin PTE_U para evitar fallos de ejecución en S-mode)
    map_range(
        unsafe { &mut KERNEL_PGTABLE },
        stext_addr,
        stext_addr,
        etext_addr - stext_addr,
        PTE_R | PTE_X,
        &mut allocator,
    );

    // 4b. Mapeos duplicados para U-mode (offset de -0x40000000)
    // Mapear OpenSBI para U-mode
    map_range(
        unsafe { &mut KERNEL_PGTABLE },
        0x40000000,
        0x80000000,
        0x200000,
        PTE_R | PTE_X | PTE_U,
        &mut allocator,
    );

    // Mapear .text para U-mode
    map_range(
        unsafe { &mut KERNEL_PGTABLE },
        stext_addr - 0x40000000,
        stext_addr,
        etext_addr - stext_addr,
        PTE_R | PTE_X | PTE_U,
        &mut allocator,
    );

    // Mapear .rodata para U-mode
    map_range(
        unsafe { &mut KERNEL_PGTABLE },
        srodata_addr - 0x40000000,
        srodata_addr,
        erodata_addr - srodata_addr,
        PTE_R | PTE_U,
        &mut allocator,
    );

    // Mapear .data/BSS/Heap y resto de la RAM fisica para U-mode
    map_range(
        unsafe { &mut KERNEL_PGTABLE },
        sdata_addr - 0x40000000,
        sdata_addr,
        0x88000000 - sdata_addr,
        PTE_R | PTE_W | PTE_U,
        &mut allocator,
    );

    // 5. Mapeo Kernel .rodata - R-U
    map_range(
        unsafe { &mut KERNEL_PGTABLE },
        srodata_addr,
        srodata_addr,
        erodata_addr - srodata_addr,
        PTE_R | PTE_U,
        &mut allocator,
    );

    // 6. Mapeo Kernel .data/BSS/Heap y resto de la RAM fisica (hasta 0x8800_0000) - R-W-U
    map_range(
        unsafe { &mut KERNEL_PGTABLE },
        sdata_addr,
        sdata_addr,
        0x88000000 - sdata_addr,
        PTE_R | PTE_W | PTE_U,
        &mut allocator,
    );

    // Habilitar la memoria virtual cargando satp y vaciando TLB
    let root_table_pa = unsafe { &KERNEL_PGTABLE as *const PageTable as usize };
    sbi::print_str("[Paging] Cargando satp con la tabla raiz en: ");
    sbi::print_hex(root_table_pa);
    sbi::print_str("\n");

    unsafe {
        enable_paging(root_table_pa);
    }
    sbi::print_str("[Paging] Paginacion virtual Sv39 activada con éxito.\n");
}

pub unsafe fn enable_paging(root_table_pa: usize) {
    let mode_sv39 = 8usize;
    // satp: [Mode: 63-60] [ASID: 59-44] [PPN: 43-0]
    let satp_val = (mode_sv39 << 60) | (root_table_pa >> 12);
    // Habilitar SUM (bit 18) en sstatus para que el modo Supervisor pueda acceder a páginas PTE_U
    core::arch::asm!("csrs sstatus, {}", in(reg) (1 << 18));
    core::arch::asm!(
        "csrw satp, {}",
        "sfence.vma",
        in(reg) satp_val
    );
}
