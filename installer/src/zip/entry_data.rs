const LOCAL_HEADER_LENGTH: usize = 30;
const METHOD_STORED: u16 = 0;
const METHOD_DEFLATED: u16 = 8;

/// One entry's bytes, inflated if need be and checked against its CRC, which
/// catches a cache file that rotted after its sha256 was verified.
pub fn read(
    zip: &[u8],
    entry: &super::central_directory::Entry,
) -> Result<Vec<u8>, super::ZipError> {
    let data = compressed_bytes(zip, entry)?;
    let content = match entry.method {
        METHOD_STORED => data.to_vec(),
        METHOD_DEFLATED => {
            miniz_oxide::inflate::decompress_to_vec_with_limit(data, entry.uncompressed).map_err(
                |e| {
                    super::ZipError::new(format!(
                        "{} could not be decompressed ({:?})",
                        entry.path, e.status
                    ))
                },
            )?
        }
        other => {
            return Err(super::ZipError::new(format!(
                "{} uses compression method {other}, which the installer cannot read",
                entry.path
            )))
        }
    };

    if content.len() != entry.uncompressed {
        return Err(super::ZipError::new(format!(
            "{} is {} bytes but the package says {}",
            entry.path,
            content.len(),
            entry.uncompressed
        )));
    }
    if super::crc32::crc32(&content) != entry.crc32 {
        return Err(super::ZipError::new(format!(
            "{} is damaged (checksum mismatch); delete it from the cache and \
             download it again",
            entry.path
        )));
    }
    Ok(content)
}

/// Located through the local header's own name and extra lengths, which an
/// archiver may write differently from the central copy.
fn compressed_bytes<'a>(
    zip: &'a [u8],
    entry: &super::central_directory::Entry,
) -> Result<&'a [u8], super::ZipError> {
    let at = entry.local_offset;
    if super::central_directory::u32_at(zip, at)
        != Some(super::central_directory::SIGNATURE_LOCAL_HEADER)
    {
        return Err(super::ZipError::new(format!(
            "{} is not where this package says it is",
            entry.path
        )));
    }
    let name_length = super::central_directory::u16_at(zip, at + 26)
        .ok_or_else(super::ZipError::truncated)? as usize;
    let extra_length = super::central_directory::u16_at(zip, at + 28)
        .ok_or_else(super::ZipError::truncated)? as usize;
    let start = at + LOCAL_HEADER_LENGTH + name_length + extra_length;
    zip.get(start..start + entry.compressed)
        .ok_or_else(super::ZipError::truncated)
}
