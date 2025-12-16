/// XXX: this all feels silly when hard-coding the encoded bytes
/// wouldn't be that unreasonable .-.
pub fn stub_template_bytes() -> &'static [u8] {
    unsafe {
        match () {
            #[cfg(any(target_arch = "x86_64"))]
            _ => &__EXTRAS_STUB_TEMPLATE,
            #[cfg(not(any(target_arch = "x86_64")))]
            _ => &*(__extras_stub_template as unsafe extern "C" fn() as usize as *const [u8; 8]),
        }
    }
}

#[cfg(any(target_arch = "x86_64"))]
extern "C" {
    #[link_name = "__extras_stub_template"]
    static __EXTRAS_STUB_TEMPLATE: [u8; 8];
}
#[cfg(target_arch = "x86_64")]
core::arch::global_asm! {
    ".global {stub_template_return}",
    ".balign 8",
    "{stub_template_return}:",
    "ret",
    "nop",
    ".balign 8",
    stub_template_return = sym __EXTRAS_STUB_TEMPLATE,
}

/* XXX: once rustc is updated to 1.88.0, naked functions may be a viable alternative:
#[cfg(any(target_arch = "x86_64"))]
#[unsafe(naked)]
#[link_section = ".data"] // I wonder...
//#[no_mangle]
unsafe extern "C" fn __extras_stub_template() {
    core::arch::naked_asm!(
        "ret"
        "nop"
    );
}*/

#[cfg(not(any(target_arch = "x86_64")))]
#[inline(never)]
#[deprecated = "naive fallback, build architecture not supported"]
unsafe extern "C" fn __extras_stub_template() {}
