//! Pure monitor identity and device-state policy. No Windows calls in tests.

pub(crate) fn device_instance_from_monitor_path(path: &str) -> Option<String> {
    let prefix = path
        .strip_prefix(r"\\?\")
        .or_else(|| path.strip_prefix(r"\\.\"))?;
    let parts: Vec<_> = prefix.split('#').collect();
    if parts.len() != 4
        || !parts[0].eq_ignore_ascii_case("DISPLAY")
        || parts[1].is_empty()
        || parts[2].is_empty()
        || !parts[3].starts_with('{')
        || !parts[3].ends_with('}')
    {
        return None;
    }
    Some(format!("{}\\{}\\{}", parts[0], parts[1], parts[2]).to_ascii_uppercase())
}

pub(crate) fn monitor_problem_state(flags: u32, problem: u32) -> &'static str {
    const DN_HAS_PROBLEM: u32 = 0x400;
    const DN_STARTED: u32 = 0x8;
    if flags & DN_HAS_PROBLEM != 0 {
        if problem == 22 {
            "disabled"
        } else {
            "unknown"
        }
    } else if flags & DN_STARTED != 0 {
        "enabled"
    } else {
        "unknown"
    }
}

pub(crate) fn connection_key(instance: &str, technology: i32, connector: u32) -> String {
    format!(
        "{}|output={technology}|connector={connector}",
        instance.to_ascii_uppercase()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_interface_maps_to_exact_device_instance() {
        let path =
            r"\\?\DISPLAY#AOCB453#5&1E92325E&0&UID28929#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}";
        assert_eq!(
            device_instance_from_monitor_path(path).as_deref(),
            Some(r"DISPLAY\AOCB453\5&1E92325E&0&UID28929")
        );
    }

    #[test]
    fn unrelated_or_incomplete_paths_do_not_guess_a_monitor() {
        for path in [
            "",
            r"\\?\USB#AOCB453#instance#{guid}",
            r"\\?\DISPLAY#AOCB453##{guid}",
            r"\\?\DISPLAY#AOCB453#instance",
        ] {
            assert_eq!(device_instance_from_monitor_path(path), None);
        }
    }

    #[test]
    fn only_code_22_with_problem_flag_is_disabled() {
        assert_eq!(monitor_problem_state(0x400, 22), "disabled");
        assert_eq!(monitor_problem_state(0x400, 10), "unknown");
        assert_eq!(monitor_problem_state(0, 22), "unknown");
        assert_eq!(monitor_problem_state(0x8, 0), "enabled");
    }

    #[test]
    fn connection_key_is_case_insensitive_and_not_gdi_number() {
        assert_eq!(
            connection_key(r"display\aoc\unit", 5, 1),
            connection_key(r"DISPLAY\AOC\UNIT", 5, 1)
        );
        assert_ne!(
            connection_key(r"DISPLAY\AOC\UNIT", 5, 1),
            connection_key(r"DISPLAY\AOC\OTHER", 5, 1)
        );
        assert_ne!(
            connection_key(r"DISPLAY\AOC\UNIT", 5, 1),
            connection_key(r"DISPLAY\AOC\UNIT", 5, 2)
        );
    }
}
