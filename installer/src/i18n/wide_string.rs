/// NUL-terminated UTF-16, as every `-W` API takes it.
pub fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(core::iter::once(0)).collect()
}
