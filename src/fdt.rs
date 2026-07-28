// Parser zero-allocation para el Flattened Device Tree (FDT) pasada por OpenSBI
use crate::sbi;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FdtHeader {
    pub magic: u32,             // Magic number (0xd00dfeed)
    pub totalsize: u32,         // Tamaño total del FDT blob
    pub off_dt_struct: u32,     // Offset a la sección de estructura
    pub off_dt_strings: u32,    // Offset a la sección de cadenas de caracteres
    pub off_mem_rsvmap: u32,    // Offset al mapa de memoria reservada
    pub version: u32,           // Versión de la especificación
    pub last_comp_version: u32, // Última versión compatible
    pub boot_cpuid_phys: u32,   // ID físico de la CPU de arranque
    pub size_dt_strings: u32,   // Tamaño de la sección de cadenas
    pub size_dt_struct: u32,    // Tamaño de la sección de estructura
}

impl FdtHeader {
    pub unsafe fn from_ptr(ptr: usize) -> Option<&'static FdtHeader> {
        let header = &*(ptr as *const FdtHeader);
        if u32::from_be(header.magic) == 0xd00dfeed {
            Some(header)
        } else {
            None
        }
    }
}

// Auxiliar para leer strings null-terminated en memoria
unsafe fn read_string(ptr: *const u8) -> &'static str {
    let mut len = 0;
    while *ptr.add(len) != 0 {
        len += 1;
    }
    let slice = core::slice::from_raw_parts(ptr, len);
    core::str::from_utf8_unchecked(slice)
}

// Analizador y formateador del FDT
pub unsafe fn parse_fdt(fdt_ptr: usize) {
    sbi::print_str("\n[FDT] Analizando Device Tree en la dirección física: ");
    sbi::print_hex(fdt_ptr);
    sbi::print_str("\n");

    let header = match FdtHeader::from_ptr(fdt_ptr) {
        Some(h) => h,
        None => {
            sbi::print_str("[FDT ERROR] Magic number inválido en el Device Tree!\n");
            return;
        }
    };

    let totalsize = u32::from_be(header.totalsize) as usize;
    let struct_offset = u32::from_be(header.off_dt_struct) as usize;
    let strings_offset = u32::from_be(header.off_dt_strings) as usize;

    sbi::print_str("[FDT] Tamaño del blob: ");
    sbi::print_hex(totalsize);
    sbi::print_str(" bytes\n");

    let mut struct_ptr = (fdt_ptr + struct_offset) as *const u32;
    let strings_ptr = (fdt_ptr + strings_offset) as *const u8;
    let mut depth: usize = 0;

    loop {
        let token = u32::from_be(*struct_ptr);
        struct_ptr = struct_ptr.add(1);

        match token {
            1 => { // FDT_BEGIN_NODE
                let name_ptr = struct_ptr as *const u8;
                let name = read_string(name_ptr);
                
                // Avanzar struct_ptr pasada la cadena y alinear a 4 bytes
                let len = name.len() + 1;
                let aligned_words = (len + 3) / 4;
                struct_ptr = struct_ptr.add(aligned_words);

                // Indentación por nivel de profundidad
                for _ in 0..depth {
                    sbi::print_str("  ");
                }
                sbi::print_str("- Nodo: ");
                if name.is_empty() {
                    sbi::print_str("/");
                } else {
                    sbi::print_str(name);
                }
                sbi::print_str("\n");

                depth += 1;
            }
            2 => { // FDT_END_NODE
                depth = depth.saturating_sub(1);
            }
            3 => { // FDT_PROP
                let len = u32::from_be(*struct_ptr) as usize;
                struct_ptr = struct_ptr.add(1);
                let nameoff = u32::from_be(*struct_ptr) as usize;
                struct_ptr = struct_ptr.add(1);

                let prop_name = read_string(strings_ptr.add(nameoff));
                
                // Mostrar propiedades
                for _ in 0..depth {
                    sbi::print_str("  ");
                }
                sbi::print_str("  * ");
                sbi::print_str(prop_name);
                sbi::print_str(": ");

                if len > 0 {
                    let val_ptr = struct_ptr as *const u8;
                    // Detectar si los datos representan un string ASCII imprimible
                    let mut is_string = true;
                    for i in 0..(len - 1) {
                        let c = *val_ptr.add(i);
                        if c < 32 || c > 126 {
                            is_string = false;
                            break;
                        }
                    }
                    if *val_ptr.add(len - 1) != 0 {
                        is_string = false;
                    }

                    if is_string && len > 1 {
                        let val_str = core::str::from_utf8_unchecked(core::slice::from_raw_parts(val_ptr, len - 1));

                        if prop_name == "bootargs" {
                            unsafe {
                                if val_str.contains("loglevel=0") || val_str.contains("quiet") {
                                    crate::task::LOG_LEVEL = 0;
                                } else if val_str.contains("loglevel=1") {
                                    crate::task::LOG_LEVEL = 1;
                                } else if val_str.contains("loglevel=2") {
                                    crate::task::LOG_LEVEL = 2;
                                } else if val_str.contains("loglevel=3") || val_str.contains("debug") {
                                    crate::task::LOG_LEVEL = 3;
                                    crate::task::DEBUG_LOGS = true;
                                }
                            }
                        }

                        if prop_name == "bootargs" && val_str.contains("debug") {
                            unsafe {
                               crate::task::DEBUG_LOGS = true;
                            }
                        }

                        sbi::print_str("\"");
                        sbi::print_str(val_str);
                        sbi::print_str("\"");
                    } else if len == 4 {
                        let val = u32::from_be(*(struct_ptr as *const u32));
                        sbi::print_hex(val as usize);
                    } else if len == 8 {
                        let val = u64::from_be(*(struct_ptr as *const u64));
                        sbi::print_hex(val as usize);
                    } else {
                        sbi::print_str("<datos_binarios>");
                    }
                } else {
                    sbi::print_str("<vacio>");
                }
                sbi::print_str("\n");

                // Avanzar puntero pasado el valor alineado a 4 bytes
                let aligned_words = (len + 3) / 4;
                struct_ptr = struct_ptr.add(aligned_words);
            }
            4 => {} // FDT_NOP
            9 => { // FDT_END
                sbi::print_str("[FDT] Análisis del árbol de dispositivos finalizado.\n\n");
                break;
            }
            _ => {
                sbi::print_str("[FDT ERROR] Token no reconocido: ");
                sbi::print_hex(token as usize);
                sbi::print_str("\n");
                break;
            }
        }
    }
}
