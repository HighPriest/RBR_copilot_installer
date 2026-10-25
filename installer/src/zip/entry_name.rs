/// An entry name as a relative `/`-separated path that cannot leave the target,
/// or `None` for a name that tries: any `..` segment, a leading separator, a
/// `:` (a drive or an alternate data stream), a NUL, or nothing at all. Names
/// Windows would quietly rewrite go too: a trailing dot or space, and a
/// reserved device name such as `aux.ini`.
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
            _ if segment.ends_with('.') || segment.ends_with(' ') => return None,
            _ if is_reserved_device(segment) => return None,
            _ => segments.push(segment),
        }
    }
    if segments.is_empty() {
        return None;
    }
    Some(segments.join("/"))
}

/// Decided on the stem, since Windows opens `NUL.txt` as the device just as it does `NUL`.
fn is_reserved_device(segment: &str) -> bool {
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let stem = segment.split('.').next().unwrap_or(segment);
    RESERVED
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
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
