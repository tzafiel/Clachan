#[cfg(not(all(not(target_arch = "wasm32"), debug_assertions)))]
use macroquad::file::load_file;
use macroquad::prelude::{FilterMode, ImageFormat, Texture2D};

const UI_PATH: &str = "assets/ui.png";
const UI_SIZE: (u32, u32) = (128, 64);

pub struct AssetBank {
    pub ui: Texture2D,
    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
    modified: std::time::SystemTime,
    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
    poll_elapsed: f32,
}

impl AssetBank {
    pub async fn load() -> Result<Self, String> {
        Ok(Self {
            ui: load_ui_texture().await?,
            #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
            modified: modified_time()?,
            #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
            poll_elapsed: 0.0,
        })
    }

    pub async fn poll_hot_reload(&mut self, dt: f32, force: bool) -> Result<bool, String> {
        #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
        {
            self.poll_elapsed += dt;
            if !force && self.poll_elapsed < 0.5 {
                return Ok(false);
            }
            self.poll_elapsed = 0.0;
            let modified = modified_time()?;
            if force || modified != self.modified {
                self.ui = load_ui_texture().await?;
                self.modified = modified;
                return Ok(true);
            }
            Ok(false)
        }

        #[cfg(not(all(not(target_arch = "wasm32"), debug_assertions)))]
        {
            let _ = (dt, force);
            Ok(false)
        }
    }
}

async fn load_ui_texture() -> Result<Texture2D, String> {
    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
    let bytes = std::fs::read(UI_PATH).map_err(|error| format!("{UI_PATH}: {error}"))?;
    #[cfg(not(all(not(target_arch = "wasm32"), debug_assertions)))]
    let bytes = load_file(UI_PATH)
        .await
        .map_err(|error| format!("Failed loading {UI_PATH}: {error}"))?;

    validate_png(&bytes)?;
    let texture = Texture2D::from_file_with_format(&bytes, Some(ImageFormat::Png));
    texture.set_filter(FilterMode::Nearest);
    Ok(texture)
}

fn validate_png(bytes: &[u8]) -> Result<(), String> {
    const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
    if bytes.len() < 24 || bytes[..8] != PNG_SIGNATURE {
        return Err(format!("{UI_PATH} is not a valid PNG file"));
    }
    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    if (width, height) != UI_SIZE {
        return Err(format!(
            "{UI_PATH} must be {}x{} pixels (got {}x{})",
            UI_SIZE.0, UI_SIZE.1, width, height
        ));
    }
    Ok(())
}

#[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
fn modified_time() -> Result<std::time::SystemTime, String> {
    std::fs::metadata(UI_PATH)
        .and_then(|metadata| metadata.modified())
        .map_err(|error| format!("{UI_PATH}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_sprite_sheet_has_expected_dimensions() {
        validate_png(&std::fs::read(UI_PATH).expect("UI texture exists"))
            .expect("UI texture dimensions");
    }
}
