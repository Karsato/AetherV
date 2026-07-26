#[inline(always)]
pub fn sbi_putchar(c: usize) {
    unsafe {
        core::arch::asm!(
            "li a7, 1",
            "ecall",
            in("a0") c,
            out("a7") _,
        );
    }
}

pub fn print_str(s: &str) {
    for byte in s.bytes() {
        sbi_putchar(byte as usize);
    }
}

#[inline(always)]
pub fn sbi_set_timer(time: u64) {
    unsafe {
        core::arch::asm!(
            "li a7, 0x54494D45", // Timer Extension ID
            "li a6, 0",          // Set Timer Function ID
            "ecall",
            in("a0") time,
            out("a7") _,
            out("a6") _,
        );
    }
}

#[inline(always)]
pub fn get_time() -> u64 {
    let time: usize;
    unsafe {
        core::arch::asm!("csrr {}, time", out(reg) time);
    }
    time as u64
}

pub fn print_hex(mut val: usize) {
    let mut buf = [0u8; 18];
    buf[0] = b'0';
    buf[1] = b'x';
    for i in (2..18).rev() {
        let nibble = (val & 0xF) as u8;
        buf[i] = match nibble {
            0..=9 => b'0' + nibble,
            10..=15 => b'a' + (nibble - 10),
            _ => unreachable!(),
        };
        val >>= 4;
    }
    if let Ok(s) = core::str::from_utf8(&buf) {
        print_str(s);
    }
}
