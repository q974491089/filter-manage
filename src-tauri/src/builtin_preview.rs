use base64::Engine;
use std::io::Cursor;

const MAX_ICC_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    brightness: i32,
    contrast: i32,
    gamma: f64,
    digital_vibrance: i32,
    rgb_r: i32,
    rgb_g: i32,
    rgb_b: i32,
}

impl Settings {
    fn native(&self) -> crate::nvidia::NvidiaSettings {
        crate::nvidia::NvidiaSettings {
            brightness: self.brightness.clamp(-125, 125),
            contrast: self.contrast.clamp(-82, 82),
            gamma: self.gamma,
            digital_vibrance: self.digital_vibrance,
            rgb_r: self.rgb_r,
            rgb_g: self.rgb_g,
            rgb_b: self.rgb_b,
        }
    }
}

fn scene_bytes(scene: &str) -> Result<&'static [u8], String> {
    match scene {
        "deltaforce" => Ok(include_bytes!("../resources/game-previews/deltaforce.png")),
        "cs2" => Ok(include_bytes!("../resources/game-previews/cs2.png")),
        "valorant" => Ok(include_bytes!("../resources/game-previews/valorant.png")),
        // 旧版分享/投稿保存的场景 id，按画面对应到新场景。
        "siege" => scene_bytes("deltaforce"),
        "solarium" => scene_bytes("cs2"),
        "capture" => scene_bytes("valorant"),
        _ => Err("请选择内置游戏原图".into()),
    }
}

fn decode_icc(encoded: &str) -> Result<Option<[[u16; 256]; 3]>, String> {
    if encoded.len() > MAX_ICC_BYTES.div_ceil(3) * 4 {
        return Err("ICC 文件不能超过 2 MB".into());
    }
    let data = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "ICC 文件编码无效")?;
    if data.len() > MAX_ICC_BYTES {
        return Err("ICC 文件不能超过 2 MB".into());
    }
    let has_curve = crate::icc_validation::validate_profile_tags(&data)?;
    if has_curve {
        crate::icc::parse_vcgt(&data)
            .map(Some)
            .ok_or_else(|| "ICC 曲线数据无效".into())
    } else {
        Ok(None)
    }
}

fn render(scene: &str, curve: Option<&[[u16; 256]; 3]>, settings: &Settings) -> Result<Vec<u8>, String> {
    if !settings.gamma.is_finite() {
        return Err("Gamma 数值无效".into());
    }
    let decoder = png::Decoder::new(Cursor::new(scene_bytes(scene)?));
    let mut reader = decoder.read_info().map_err(|e| format!("读取内置原图失败：{e}"))?;
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut pixels).map_err(|e| format!("解码内置原图失败：{e}"))?;
    if info.color_type != png::ColorType::Rgb || info.bit_depth != png::BitDepth::Eight {
        return Err("内置原图格式无效".into());
    }
    pixels.truncate(info.buffer_size());
    let ramp = crate::nvidia::compose_gamma_ramp(&settings.native(), curve);
    let saturation = settings.digital_vibrance.clamp(0, 100) as f64 / 50.0;
    for pixel in pixels.chunks_exact_mut(3) {
        let values = [
            ramp[pixel[0] as usize] as f64 / 257.0,
            ramp[256 + pixel[1] as usize] as f64 / 257.0,
            ramp[512 + pixel[2] as usize] as f64 / 257.0,
        ];
        let luminance = values[0] * 0.2126 + values[1] * 0.7152 + values[2] * 0.0722;
        for channel in 0..3 {
            pixel[channel] = (luminance + (values[channel] - luminance) * saturation)
                .round().clamp(0.0, 255.0) as u8;
        }
    }
    let mut output = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut output, info.width, info.height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| format!("生成预览失败：{e}"))?;
        writer.write_image_data(&pixels).map_err(|e| format!("编码预览失败：{e}"))?;
    }
    Ok(output)
}

// Pure image rendering: no desktop capture, window hiding or display writes.
#[tauri::command]
pub async fn render_builtin_icc_preview(
    scene_id: String,
    icc_base64: Option<String>,
    profile_name: Option<String>,
    settings: Settings,
) -> Result<String, String> {
    scene_bytes(&scene_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let curve = if let Some(encoded) = icc_base64.filter(|value| !value.is_empty()) {
            decode_icc(&encoded)?
        } else if let Some(name) = profile_name.filter(|value| !value.is_empty() && value != "Default" && value != "Default.icc") {
            crate::icc::read_profile_curve(&name)?
        } else {
            None
        };
        let image = render(&scene_id, curve.as_ref(), &settings)?;
        Ok(base64::engine::general_purpose::STANDARD.encode(image))
    })
    .await
    .map_err(|e| format!("预览任务失败：{e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn neutral() -> Settings {
        Settings { brightness: 0, contrast: 0, gamma: 1.0, digital_vibrance: 50, rgb_r: 0, rgb_g: 0, rgb_b: 0 }
    }

    fn pixels(bytes: &[u8]) -> Vec<u8> {
        let mut reader = png::Decoder::new(Cursor::new(bytes)).read_info().unwrap();
        let mut out = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut out).unwrap();
        out.truncate(info.buffer_size());
        out
    }

    #[test]
    fn neutral_preview_preserves_each_builtin_original() {
        for scene in ["deltaforce", "cs2", "valorant"] {
            let source = pixels(scene_bytes(scene).unwrap());
            let result = pixels(&render(scene, None, &neutral()).unwrap());
            assert_eq!(source, result, "{scene}");
        }
    }

    #[test]
    fn settings_and_icc_change_only_pixels_from_the_fixed_original() {
        let mut settings = neutral();
        settings.gamma = 2.0;
        let base = pixels(scene_bytes("valorant").unwrap());
        let brighter = pixels(&render("valorant", None, &settings).unwrap());
        assert_eq!(base.len(), brighter.len());
        assert!(base.iter().zip(&brighter).any(|(a,b)| b > a));
        assert!(base.iter().zip(&brighter).all(|(a,b)| b >= a));
        let black = [[0u16; 256]; 3];
        assert!(pixels(&render("valorant", Some(&black), &neutral()).unwrap()).iter().all(|v| *v == 0));
    }

    #[test]
    fn arbitrary_scene_paths_and_invalid_icc_are_rejected() {
        assert!(scene_bytes("../secret.png").is_err());
        assert!(scene_bytes("https://example.com/image.png").is_err());
        assert!(decode_icc("aGVsbG8=").is_err());
        let mut bad = neutral();
        bad.gamma = f64::NAN;
        assert!(render("deltaforce", None, &bad).is_err());
    }
}
