/// An entry name as a relative `/`-separated path that cannot leave the target,
/// or `None` for a name that tries: any `..` segment, a leading separator, a
/// `:` (a drive or an alternate data stream), a NUL, or nothing at all.
///
/// `\` is a separator too: Windows archivers write it, and treating it as a
/// character would both mis-name files and let `..\..\x` through.
pub fn safe_relative(name: &str) -> Option<String> {
    if name.starts_with('/') || name.starts_with('\\') {
        return None;
    }
    let mut segments: Vec<&str> = Vec::new();
    for segment in name.split(['/', '\\']) {
        match segment {
            "" | "." => continue,
            ".." => return None,
            _ if segment.contains(':') || segment.contains('\0') => return None,
            _ => segments.push(segment),
        }
    }
    if segments.is_empty() {
        return None;
    }
    Some(segments.join("/"))
}

/// UTF-8 when general-purpose flag bit 11 says so, else CP437 as the spec and
/// .NET have it, so a file is written under the name the game expects.
pub fn decode_name(raw: &[u8], flags: u16) -> String {
    const UTF8_FLAG: u16 = 1 << 11;
    if flags & UTF8_FLAG != 0 {
        if let Ok(text) = std::str::from_utf8(raw) {
            return text.to_string();
        }
    }
    raw.iter()
        .map(|&byte| {
            if byte < 0x80 {
                byte as char
            } else {
                CP437_HIGH[(byte - 0x80) as usize]
            }
        })
        .collect()
}

/// CP437 code points 0x80-0xFF; the low half is ASCII.
#[rustfmt::skip]
const CP437_HIGH: [char; 128] = [
    'Ç','ü','é','â','ä','à','å','ç','ê','ë','è','ï','î','ì','Ä','Å',
    'É','æ','Æ','ô','ö','ò','û','ù','ÿ','Ö','Ü','¢','£','¥','₧','ƒ',
    'á','í','ó','ú','ñ','Ñ','ª','º','¿','⌐','¬','½','¼','¡','«','»',
    '░','▒','▓','│','┤','╡','╢','╖','╕','╣','║','╗','╝','╜','╛','┐',
    '└','┴','┬','├','─','┼','╞','╟','╚','╔','╩','╦','╠','═','╬','╧',
    '╨','╤','╥','╙','╘','╒','╓','╫','╪','┘','┌','█','▄','▌','▐','▀',
    'α','ß','Γ','π','Σ','σ','µ','τ','Φ','Θ','Ω','δ','∞','φ','ε','∩',
    '≡','±','≥','≤','⌠','⌡','÷','≈','°','∙','·','√','ⁿ','²','■',' ',
];
