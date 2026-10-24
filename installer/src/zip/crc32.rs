/// CRC-32 with the zip polynomial, a nibble at a time: 64 bytes of table
/// instead of 1 KiB, at a cost in speed invisible at package sizes.
pub fn crc32(data: &[u8]) -> u32 {
    const TABLE: [u32; 16] = [
        0x0000_0000,
        0x1db7_1064,
        0x3b6e_20c8,
        0x26d9_30ac,
        0x76dc_4190,
        0x6b6b_51f4,
        0x4db2_6158,
        0x5005_713c,
        0xedb8_8320,
        0xf00f_9344,
        0xd6d6_a3e8,
        0xcb61_b38c,
        0x9b64_c2b0,
        0x86d3_d2d4,
        0xa00a_e278,
        0xbdbd_f21c,
    ];
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        crc = (crc >> 4) ^ TABLE[(crc & 0x0F) as usize];
        crc = (crc >> 4) ^ TABLE[(crc & 0x0F) as usize];
    }
    !crc
}
