//! ICC tag-boundary validation, kept separate from curve interpolation.

fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(
        data[offset..offset + 4]
            .try_into()
            .expect("validated ICC field"),
    )
}

/// True means a decodable VCGT is present. Malformed/unsupported VCGT is an
/// error, never the successful absence of a filter curve.
pub(crate) fn validate_profile_tags(bytes: &[u8]) -> Result<bool, String> {
    if bytes.len() < 132 || bytes.get(36..40) != Some(b"acsp") {
        return Err("ICC 文件头无效".into());
    }
    let declared = read_u32(bytes, 0) as usize;
    let count = read_u32(bytes, 128) as usize;
    let table_end = count
        .checked_mul(12)
        .and_then(|size| size.checked_add(132))
        .ok_or("ICC 标签表长度溢出")?;
    if declared < 132 || declared > bytes.len() || table_end > declared {
        return Err("ICC 文件或标签表已截断".into());
    }
    let mut has_vcgt = false;
    for index in 0..count {
        let entry = 132 + index * 12;
        let offset = read_u32(bytes, entry + 4) as usize;
        let size = read_u32(bytes, entry + 8) as usize;
        let end = offset.checked_add(size).ok_or("ICC 标签长度溢出")?;
        if offset < table_end || size < 8 || end > declared {
            return Err(format!("ICC 标签 {index} 的数据边界无效"));
        }
        if &bytes[entry..entry + 4] == b"vcgt" {
            if has_vcgt {
                return Err("ICC 包含重复 VCGT 标签".into());
            }
            validate_vcgt(&bytes[offset..end])?;
            has_vcgt = true;
        }
    }
    Ok(has_vcgt)
}

fn validate_vcgt(data: &[u8]) -> Result<(), String> {
    if data.len() < 12 || &data[..4] != b"vcgt" {
        return Err("VCGT 标签头无效".into());
    }
    match read_u32(data, 8) {
        0 => {
            if data.len() < 18 {
                return Err("VCGT 曲线表已截断".into());
            }
            let short =
                |offset: usize| u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
            let (channels, entries, width) = (short(12), short(14), short(16));
            if channels < 3 || entries == 0 || ![1, 2].contains(&width) {
                return Err("VCGT 曲线通道、条目或精度不受支持".into());
            }
            let required = channels
                .checked_mul(entries)
                .and_then(|n| n.checked_mul(width))
                .and_then(|n| n.checked_add(18))
                .ok_or("VCGT 曲线表长度溢出")?;
            if required > data.len() {
                return Err("VCGT 曲线数据超出标签声明长度".into());
            }
        }
        1 => {
            if data.len() < 48 {
                return Err("VCGT 公式已截断".into());
            }
            for channel in 0..3 {
                let base = 12 + channel * 12;
                let fixed = |offset| read_u32(data, offset) as i32 as f64 / 65536.0;
                if fixed(base) <= 0.0 || fixed(base + 8) <= fixed(base + 4) {
                    return Err("VCGT 公式参数无效".into());
                }
            }
        }
        kind => return Err(format!("不支持 VCGT 编码 {kind}")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(tag: Option<(&[u8; 4], &[u8])>) -> Vec<u8> {
        let header_size = if tag.is_some() { 144 } else { 132 };
        let mut data = vec![0; header_size];
        data[36..40].copy_from_slice(b"acsp");
        if let Some((signature, payload)) = tag {
            data[128..132].copy_from_slice(&1u32.to_be_bytes());
            data[132..136].copy_from_slice(signature);
            data[136..140].copy_from_slice(&144u32.to_be_bytes());
            data[140..144].copy_from_slice(&(payload.len() as u32).to_be_bytes());
            data.extend_from_slice(payload);
        }
        let length = data.len() as u32;
        data[..4].copy_from_slice(&length.to_be_bytes());
        data
    }

    fn vcgt_table() -> Vec<u8> {
        let mut payload = vec![0; 18 + 3 * 4 * 2];
        payload[..4].copy_from_slice(b"vcgt");
        payload[12..14].copy_from_slice(&3u16.to_be_bytes());
        payload[14..16].copy_from_slice(&4u16.to_be_bytes());
        payload[16..18].copy_from_slice(&2u16.to_be_bytes());
        payload
    }

    #[test]
    fn absent_curve_is_distinct_from_truncated_curve() {
        assert_eq!(validate_profile_tags(&profile(None)).unwrap(), false);
        let payload = vcgt_table();
        assert_eq!(
            validate_profile_tags(&profile(Some((b"vcgt", &payload)))).unwrap(),
            true
        );
        assert!(validate_profile_tags(&profile(Some((b"vcgt", &payload[..25])))).is_err());
    }

    #[test]
    fn tag_cannot_read_bytes_beyond_its_declared_extent() {
        let mut data = profile(Some((b"vcgt", &vcgt_table())));
        data[140..144].copy_from_slice(&18u32.to_be_bytes());
        assert!(validate_profile_tags(&data).is_err());
    }

    #[test]
    fn tag_range_and_header_overlap_are_rejected() {
        let mut data = profile(Some((b"vcgt", &vcgt_table())));
        data[136..140].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(validate_profile_tags(&data).is_err());
        data[136..140].copy_from_slice(&128u32.to_be_bytes());
        assert!(validate_profile_tags(&data).is_err());
    }

    #[test]
    fn invalid_formula_and_unsupported_encoding_are_not_absent() {
        let mut payload = vec![0; 48];
        payload[..4].copy_from_slice(b"vcgt");
        payload[8..12].copy_from_slice(&1u32.to_be_bytes());
        assert!(validate_profile_tags(&profile(Some((b"vcgt", &payload)))).is_err());
        payload[8..12].copy_from_slice(&2u32.to_be_bytes());
        assert!(validate_profile_tags(&profile(Some((b"vcgt", &payload)))).is_err());
    }
}
