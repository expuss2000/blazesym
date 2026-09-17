//! Support for reading of GNU .`debug_sup` data as prescribed in DWARF v.5.
//!
//! .`debug_sup` is a special section of executable files that should contain:
//!   - `version`: u16 representing the version of the DWARF information for the
//!     compilation unit
//!   - `is_supplementary`: u8 which is set to 1 if the file in which the
//!     section is stored is a supplementary file, 0 o.w.
//!   - `sup_filename`: null-terminated supplementary file filename (is
//!     !`is_supplementary`)
//!   - `sup_checksum_len`: uleb128 indicating the length of the following
//!     checksum field
//!   - `sup_checksum`: [u8: `sup_checksum_len`] known as `build_id`

use std::ffi::OsStr;

use crate::elf::ElfParser;
use crate::error::IntoError as _;
use crate::util::bytes_to_os_str;
use crate::util::ReadRaw as _;
use crate::Result;


/// Read the debug sup section located in non-supplementary debug files.
pub(crate) fn read_debug_suplink(parser: &ElfParser) -> Result<Option<(&OsStr, &[u8])>> {
    let debug_suplink_section = ".debug_sup";
    let idx = if let Ok(Some(idx)) = parser.find_section(debug_suplink_section) {
        idx
    } else {
        return Ok(None)
    };

    // SANITY: We just found the index so the section should always be
    //         found.
    let data = parser.section_data(idx).unwrap();
    let (_, is_supplementary, file, build_id) =
        parse_debug_suplink_section_data(data).unwrap().unwrap();

    if is_supplementary {
        return Ok(None)
    }

    Ok(Some((file, build_id)))
}

// Read the debug sup section located in supplementary debug files.
pub(crate) fn read_debug_sup(parser: &ElfParser) -> Result<Option<&[u8]>> {
    let debug_suplink_section = ".debug_sup";
    let idx = if let Ok(Some(idx)) = parser.find_section(debug_suplink_section) {
        idx
    } else {
        return Ok(None)
    };

    // SANITY: We just found the index so the section should always be
    //         found.
    let data = parser.section_data(idx).unwrap();
    let (_, is_supplementary, _, build_id) =
        parse_debug_suplink_section_data(data).unwrap().unwrap();

    if !is_supplementary {
        return Ok(None)
    }

    Ok(Some(build_id))
}

// Parse a `.debug_sup` section
fn parse_debug_suplink_section_data(mut data: &[u8]) -> Result<Option<(u16, bool, &OsStr, &[u8])>> {
    let version = data.read_u16().unwrap();
    println!("Version found in .debug_section: {version}");

    let is_supplementary = match data.read_u8().unwrap() {
        0 => false,
        1 => true,
        v => panic!("Invalid is_supplementary field value: {v}"),
    };

    let file = data
        .read_cstr()
        .ok_or_invalid_data(|| "failed to read .debug_sup file name")?;
    let file = bytes_to_os_str(file.to_bytes())?;

    let sup_checksum_len = data.read_u64_leb128().unwrap();

    let sup_checksum = data
        .read_slice(sup_checksum_len.try_into().unwrap())
        .ok_or_invalid_data(|| "failed to read .debug_sup checksum")?;

    Ok(Some((version, is_supplementary, file, sup_checksum)))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::mem::size_of_val;
    use std::slice;

    use test_tag::tag;


    /// Check that we can correctly read a build id from debug altlink section
    /// data.
    #[tag(miri)]
    #[test]
    fn unaligned_debug_suplink_parsing() {
        let section_data = [
            0x5, 0x0, 0x0, b'.', b'.', b'/', b'.', b'.', b'/', b'.', b'd', b'w', b'z', b'/', b'p',
            b'r', b'o', b'g', b'r', b'a', b'm', 0x0, 0x14, 0x7f, 0xd3, 0x76, 0x0a, 0xf3, 0x98,
            0xa1, 0xcc, 0x3f, 0x90, 0x45, 0x69, 0x9a, 0xda, 0x29, 0xe0, 0xb6, 0x6b, 0x45, 0xc8,
        ];

        let mut buffer = [0u64; 8];
        let buffer = unsafe {
            slice::from_raw_parts_mut(
                buffer.as_mut_ptr().cast::<u8>(),
                buffer.len() * size_of_val(&buffer[0]),
            )
        };

        // Make the buffer unaligned.
        let buffer = &mut buffer[3..3 + section_data.len()];
        // Now write the section data into it.
        let () = buffer.copy_from_slice(&section_data);
        println!("unaligned buffer: {buffer:#?}");

        let (version, is_supplementary, file, build_id) =
            parse_debug_suplink_section_data(buffer).unwrap().unwrap();

        assert_eq!(version, 5);
        assert!(!is_supplementary);
        assert_eq!(file, OsStr::new("../../.dwz/program"));
        assert_eq!(
            build_id,
            [
                0x7f, 0xd3, 0x76, 0x0a, 0xf3, 0x98, 0xa1, 0xcc, 0x3f, 0x90, 0x45, 0x69, 0x9a, 0xda,
                0x29, 0xe0, 0xb6, 0x6b, 0x45, 0xc8
            ]
        );
    }
}
