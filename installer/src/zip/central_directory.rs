const SIGNATURE_END_OF_DIRECTORY: u32 = 0x0605_4b50;
const SIGNATURE_DIRECTORY_ENTRY: u32 = 0x0201_4b50;
pub const SIGNATURE_LOCAL_HEADER: u32 = 0x0403_4b50;
const END_OF_DIRECTORY_LENGTH: usize = 22;
const DIRECTORY_ENTRY_LENGTH: usize = 46;
/// Zip64 saturates the 16-bit count field.
const ZIP64_COUNT: usize = 0xFFFF;
/// Zip64 saturates the 32-bit size and offset fields.
const ZIP64_FIELD: usize = 0xFFFF_FFFF;

/// One entry as the central directory describes it, which is the authority:
/// a streaming archiver leaves the local header's sizes zero.
pub struct Entry {
    /// Validated, `/`-separated, relative.
    pub path: String,
    pub is_dir: bool,
    pub method: u16,
    pub crc32: u32,
    pub compressed: usize,
    pub uncompressed: usize,
    pub local_offset: usize,
}

/// Every entry, refusing the whole archive when any name would leave the target.
pub fn read(zip: &[u8]) -> Result<Vec<Entry>, super::ZipError> {
    let end_record = find_end_of_directory(zip)?;
    let count = u16_at(zip, end_record + 10).ok_or_else(super::ZipError::truncated)? as usize;
    let size = u32_at(zip, end_record + 12).ok_or_else(super::ZipError::truncated)? as usize;
    let offset = u32_at(zip, end_record + 16).ok_or_else(super::ZipError::truncated)? as usize;
    if count == ZIP64_COUNT || size == ZIP64_FIELD || offset == ZIP64_FIELD {
        return Err(super::ZipError::new(
            "this package uses the zip64 format, which the installer cannot read",
        ));
    }
    let end = offset
        .checked_add(size)
        .filter(|&end| end <= zip.len())
        .ok_or_else(super::ZipError::truncated)?;

    let mut entries = Vec::with_capacity(count);
    let mut at = offset;
    while at < end && u32_at(zip, at) == Some(SIGNATURE_DIRECTORY_ENTRY) {
        let (entry, next) = read_entry(zip, at)?;
        entries.push(entry);
        at = next;
    }
    // A short list would become a short ledger, leaving files nothing owns.
    if entries.len() != count {
        return Err(super::ZipError::new(format!(
            "this package is damaged: it lists {count} entries but only {} could be read",
            entries.len()
        )));
    }
    Ok(keep_last_of_each_path(entries))
}

/// A zip may list one path twice, or in two cases that NTFS treats as one;
/// extraction leaves the last on disk, so the listing keeps the last too.
fn keep_last_of_each_path(entries: Vec<Entry>) -> Vec<Entry> {
    let mut seen = std::collections::BTreeSet::new();
    let mut kept: Vec<Entry> = entries
        .into_iter()
        .rev()
        .filter(|entry| seen.insert(entry.path.to_lowercase()))
        .collect();
    kept.reverse();
    kept
}

/// The entry at `at`, and where the next one starts.
fn read_entry(zip: &[u8], at: usize) -> Result<(Entry, usize), super::ZipError> {
    let field16 = |offset: usize| u16_at(zip, at + offset).ok_or_else(super::ZipError::truncated);
    let field32 = |offset: usize| u32_at(zip, at + offset).ok_or_else(super::ZipError::truncated);
    let flags = field16(8)?;
    let method = field16(10)?;
    let crc32 = field32(16)?;
    let compressed = field32(20)? as usize;
    let uncompressed = field32(24)? as usize;
    let name_length = field16(28)? as usize;
    let extra_length = field16(30)? as usize;
    let comment_length = field16(32)? as usize;
    let local_offset = field32(42)? as usize;

    let name_at = at + DIRECTORY_ENTRY_LENGTH;
    let raw_name = zip
        .get(name_at..name_at + name_length)
        .ok_or_else(super::ZipError::truncated)?;
    let name = super::entry_name::decode_name(raw_name, flags);
    // Read before normalizing, which drops the trailing separator marking a directory.
    let is_dir = name.ends_with('/') || name.ends_with('\\');
    let path = super::entry_name::safe_relative(&name).ok_or_else(|| {
        super::ZipError::new(format!(
            "this package contains an entry that would write outside the \
             game folder ({name}); it will not be installed"
        ))
    })?;

    let entry = Entry {
        path,
        is_dir,
        method,
        crc32,
        compressed,
        uncompressed,
        local_offset,
    };
    Ok((entry, name_at + name_length + extra_length + comment_length))
}

/// Scans back over the longest comment a 16-bit length allows, since the
/// end-of-directory record is the only fixed point in a zip.
fn find_end_of_directory(zip: &[u8]) -> Result<usize, super::ZipError> {
    if zip.len() < END_OF_DIRECTORY_LENGTH {
        return Err(super::ZipError::new("this package is not a zip archive"));
    }
    let latest = zip.len() - END_OF_DIRECTORY_LENGTH;
    let earliest = latest.saturating_sub(u16::MAX as usize);
    (earliest..=latest)
        .rev()
        .find(|&at| u32_at(zip, at) == Some(SIGNATURE_END_OF_DIRECTORY))
        .ok_or_else(|| super::ZipError::new("this package is not a zip archive, or it is damaged"))
}

pub fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

pub fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}
