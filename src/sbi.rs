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
