use crate::config::{PIXELS_PER_METER, TILE_SIZE_PIXELS};
use macroquad::file::{load_file, load_string};
use macroquad::prelude::{FilterMode, ImageFormat, Texture2D, Vec2};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[cfg(test)]
#[path = "ldtk_loader_tests.rs"]
mod tests;

pub const PROJECT_PATH: &str = "assets/world/clachan.ldtk";
pub const ENTITY_PACK_PATH: &str = "assets/packs/base/entities.ron";
const GRID_PIXELS_PER_METER: f32 = PIXELS_PER_METER;

#[derive(Clone, Debug, Deserialize)]
pub struct LdtkProject {
    #[serde(rename = "defaultGridSize", default)]
    pub default_grid_size: u32,
    #[serde(default)]
    pub levels: Vec<LdtkLevel>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LdtkLevel {
    pub identifier: String,
    #[serde(rename = "worldX", default)]
    pub world_x: i32,
    #[serde(rename = "worldY", default)]
    pub world_y: i32,
    #[serde(rename = "pxWid", default)]
    pub px_wid: u32,
    #[serde(rename = "pxHei", default)]
    pub px_hei: u32,
    #[serde(rename = "worldDepth", default)]
    pub world_depth: i32,
    #[serde(rename = "layerInstances", default)]
    pub layer_instances: Option<Vec<LdtkLayer>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LdtkLayer {
    #[serde(rename = "__identifier")]
    pub identifier: String,
    #[serde(rename = "__type", default)]
    pub layer_type: String,
    #[serde(rename = "__cWid", default)]
    pub columns: u32,
    #[serde(rename = "__cHei", default)]
    pub rows: u32,
    #[serde(rename = "__gridSize", default)]
    pub grid_size: u32,
    #[serde(rename = "__pxTotalOffsetX", default)]
    pub offset_x: i32,
    #[serde(rename = "__pxTotalOffsetY", default)]
    pub offset_y: i32,
    #[serde(rename = "__tilesetRelPath", default)]
    pub tileset_rel_path: Option<String>,
    #[serde(default = "visible_by_default")]
    pub visible: bool,
    #[serde(rename = "intGridCsv", default)]
    pub int_grid_csv: Vec<i32>,
    #[serde(rename = "autoLayerTiles", default)]
    pub auto_layer_tiles: Vec<LdtkTile>,
    #[serde(rename = "gridTiles", default)]
    pub grid_tiles: Vec<LdtkTile>,
    #[serde(rename = "entityInstances", default)]
    pub entity_instances: Vec<LdtkEntityInstance>,
}

fn visible_by_default() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize)]
pub struct LdtkTile {
    pub px: [f32; 2],
    pub src: [f32; 2],
    #[serde(default)]
    pub f: u8,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LdtkEntityInstance {
    #[serde(rename = "__identifier")]
    pub identifier: String,
    pub iid: String,
    pub px: [f32; 2],
    #[serde(default)]
    pub width: f32,
    #[serde(default)]
    pub height: f32,
    #[serde(rename = "__pivot", default = "default_pivot")]
    pub pivot: [f32; 2],
    #[serde(rename = "fieldInstances", default)]
    pub field_instances: Vec<Value>,
}

fn default_pivot() -> [f32; 2] {
    [0.5, 0.5]
}

#[derive(Clone, Debug, Deserialize)]
pub struct EntityPack {
    pub sprite_path: String,
    #[serde(default)]
    pub hooded_sprite_path: Option<String>,
    #[serde(default)]
    pub kilt_layer_path: Option<String>,
    #[serde(default)]
    pub cloak_layer_path: Option<String>,
    #[serde(default)]
    pub hooded_cloak_layer_path: Option<String>,
    pub frame_size: (u32, u32),
    pub foot_offset_y: f32,
    pub collider: Option<(f32, f32, f32, f32)>,
}

#[derive(Clone, Debug)]
pub struct LdtkDrawEntity {
    pub level_index: usize,
    pub layer_index: usize,
    pub entity_index: usize,
    pub world_x: f32,
    pub world_y: f32,
}

#[derive(Clone)]
pub struct LdtkCollisionWorld {
    project: LdtkProject,
    entity_packs: HashMap<String, EntityPack>,
}

impl LdtkCollisionWorld {
    pub fn is_sheltered_world_pos(&self, x: f32, y: f32, z: i32) -> bool {
        let px = x * GRID_PIXELS_PER_METER;
        let py = y * GRID_PIXELS_PER_METER;
        for level in &self.project.levels {
            if level.world_depth != z {
                continue;
            }
            let local_x = px - level.world_x as f32;
            let local_y = py - level.world_y as f32;
            if local_x < 0.0
                || local_y < 0.0
                || local_x >= level.px_wid as f32
                || local_y >= level.px_hei as f32
            {
                continue;
            }
            let Some(layers) = level.layer_instances.as_ref() else {
                continue;
            };
            for layer in layers {
                if !layer.visible {
                    continue;
                }
                let grid_size = layer.grid_size.max(1) as f32;
                let tx = local_x - layer.offset_x as f32;
                let ty = local_y - layer.offset_y as f32;

                if layer.identifier == "Shelter" {
                    let cell_x = (tx / grid_size).floor() as i32;
                    let cell_y = (ty / grid_size).floor() as i32;
                    if cell_x >= 0
                        && cell_y >= 0
                        && cell_x < layer.columns as i32
                        && cell_y < layer.rows as i32
                    {
                        let index = cell_y as usize * layer.columns as usize + cell_x as usize;
                        if layer.int_grid_csv.get(index).copied().unwrap_or_default() > 0 {
                            return true;
                        }
                    }
                } else if layer.identifier == "Overhead"
                    || layer.identifier.starts_with("Overhead_")
                {
                    if layer
                        .auto_layer_tiles
                        .iter()
                        .chain(layer.grid_tiles.iter())
                        .any(|tile| {
                            tx >= tile.px[0]
                                && tx < tile.px[0] + grid_size
                                && ty >= tile.px[1]
                                && ty < tile.px[1] + grid_size
                        })
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn is_solid_world_pos(&self, x: f32, y: f32, z: i32) -> bool {
        let px = x * GRID_PIXELS_PER_METER;
        let py = y * GRID_PIXELS_PER_METER;
        for level in &self.project.levels {
            if level.world_depth != z {
                continue;
            }
            let local_x = px - level.world_x as f32;
            let local_y = py - level.world_y as f32;
            if local_x < 0.0
                || local_y < 0.0
                || local_x >= level.px_wid as f32
                || local_y >= level.px_hei as f32
            {
                continue;
            }
            let Some(layers) = level.layer_instances.as_ref() else {
                continue;
            };
            for layer in layers
                .iter()
                .filter(|layer| layer.identifier == "Collision")
            {
                let grid_size = layer.grid_size.max(1);
                let cell_x = ((local_x - layer.offset_x as f32) / grid_size as f32).floor() as i32;
                let cell_y = ((local_y - layer.offset_y as f32) / grid_size as f32).floor() as i32;
                if cell_x >= 0
                    && cell_y >= 0
                    && cell_x < layer.columns as i32
                    && cell_y < layer.rows as i32
                {
                    let index = cell_y as usize * layer.columns as usize + cell_x as usize;
                    if layer.int_grid_csv.get(index).copied().unwrap_or_default() > 0 {
                        return true;
                    }
                }
            }

            for layer in layers {
                for entity in &layer.entity_instances {
                    let Some((offset_x, offset_y, width, height)) = self
                        .entity_packs
                        .get(&entity.identifier)
                        .and_then(|pack| pack.collider)
                    else {
                        continue;
                    };
                    let entity_x = level.world_x as f32 + layer.offset_x as f32 + entity.px[0];
                    let entity_y = level.world_y as f32 + layer.offset_y as f32 + entity.px[1];
                    let x = px;
                    let y = py;
                    if x >= entity_x + offset_x * GRID_PIXELS_PER_METER
                        && x < entity_x + (offset_x + width) * GRID_PIXELS_PER_METER
                        && y >= entity_y + offset_y * GRID_PIXELS_PER_METER
                        && y < entity_y + (offset_y + height) * GRID_PIXELS_PER_METER
                    {
                        return true;
                    }
                }
            }
        }
        false
    }
}

pub struct LdtkLoader {
    project: LdtkProject,
    entity_packs: HashMap<String, EntityPack>,
    textures: HashMap<String, Texture2D>,
    world_dir: PathBuf,
    warned_missing: HashSet<String>,
}

impl LdtkLoader {
    pub async fn load() -> Result<Self, String> {
        Self::load_with_warnings(HashSet::new()).await
    }

    async fn load_with_warnings(warned_missing: HashSet<String>) -> Result<Self, String> {
        let world_dir = Path::new(PROJECT_PATH)
            .parent()
            .ok_or_else(|| format!("{PROJECT_PATH} has no parent directory"))?
            .to_path_buf();
        let project_text = load_string(PROJECT_PATH)
            .await
            .map_err(|error| format!("Failed reading {PROJECT_PATH}: {error}"))?;
        let project: LdtkProject = serde_json::from_str(&project_text)
            .map_err(|error| format!("Failed parsing {PROJECT_PATH}: {error}"))?;
        if project.default_grid_size != TILE_SIZE_PIXELS {
            return Err(format!(
                "{PROJECT_PATH} uses a {}px grid; expected {}px",
                project.default_grid_size, TILE_SIZE_PIXELS
            ));
        }
        if project.levels.is_empty() {
            return Err(format!("{PROJECT_PATH} contains no levels"));
        }
        if project.levels.iter().any(|level| {
            level
                .layer_instances
                .as_ref()
                .map(|layers| {
                    layers
                        .iter()
                        .any(|layer| layer.grid_size != TILE_SIZE_PIXELS)
                })
                .unwrap_or(false)
        }) {
            return Err(format!(
                "{PROJECT_PATH} contains a layer that does not use a {TILE_SIZE_PIXELS}px grid"
            ));
        }

        let pack_text = load_string(ENTITY_PACK_PATH)
            .await
            .map_err(|error| format!("Failed reading {ENTITY_PACK_PATH}: {error}"))?;
        let entity_packs: HashMap<String, EntityPack> = ron::from_str(&pack_text)
            .map_err(|error| format!("Failed parsing {ENTITY_PACK_PATH}: {error}"))?;
        if entity_packs.values().any(|pack| {
            pack.frame_size.0 == 0
                || pack.frame_size.1 == 0
                || pack
                    .collider
                    .map(|(_, _, width, height)| width <= 0.0 || height <= 0.0)
                    .unwrap_or(false)
        }) {
            return Err(format!(
                "{ENTITY_PACK_PATH} contains invalid frame or collider sizes"
            ));
        }

        let mut loader = Self {
            project,
            entity_packs,
            textures: HashMap::new(),
            world_dir,
            warned_missing: warned_missing.clone(),
        };
        let paths: HashSet<String> = loader
            .entity_packs
            .values()
            .flat_map(|pack| {
                std::iter::once(pack.sprite_path.clone())
                    .chain(pack.hooded_sprite_path.clone())
                    .chain(pack.kilt_layer_path.clone())
                    .chain(pack.cloak_layer_path.clone())
                    .chain(pack.hooded_cloak_layer_path.clone())
            })
            .collect();
        for path in paths {
            loader.load_texture(&path, false).await?;
        }
        let tileset_paths: HashSet<String> = loader
            .project
            .levels
            .iter()
            .flat_map(|level| level.layer_instances.iter().flatten())
            .filter_map(|layer| layer.tileset_rel_path.clone())
            .collect();
        for path in tileset_paths {
            loader.load_texture(&path, true).await?;
        }
        Ok(loader)
    }

    pub async fn reload(&mut self) -> Result<(), String> {
        let warned_missing = self.warned_missing.clone();
        *self = Self::load_with_warnings(warned_missing).await?;
        Ok(())
    }

    pub fn project(&self) -> &LdtkProject {
        &self.project
    }

    pub fn entity_pack(&self, identifier: &str) -> Option<&EntityPack> {
        self.entity_packs.get(identifier)
    }

    pub fn light_markers(&self, z: i32, identifier: &str) -> Vec<(Vec2, f32)> {
        let mut markers = Vec::new();
        for level in &self.project.levels {
            if level.world_depth != z {
                continue;
            }
            let Some(layers) = &level.layer_instances else {
                continue;
            };
            for layer in layers.iter().filter(|layer| layer.visible) {
                for entity in layer
                    .entity_instances
                    .iter()
                    .filter(|entity| entity.identifier == identifier)
                {
                    let world_x = (level.world_x + layer.offset_x) as f32 / GRID_PIXELS_PER_METER
                        + entity.px[0] / GRID_PIXELS_PER_METER;
                    let world_y = (level.world_y + layer.offset_y) as f32 / GRID_PIXELS_PER_METER
                        + entity.px[1] / GRID_PIXELS_PER_METER;
                    let radius =
                        (entity.width.max(entity.height) / GRID_PIXELS_PER_METER * 0.5).max(2.5);
                    markers.push((Vec2::new(world_x, world_y), radius));
                }
            }
        }
        markers
    }

    pub fn texture_for_sprite(&self, identifier: &str) -> Option<&Texture2D> {
        let path = &self.entity_packs.get(identifier)?.sprite_path;
        self.textures.get(path)
    }

    pub fn texture_for_sprite_by_path(&self, path: &str) -> Option<&Texture2D> {
        self.textures.get(path)
    }

    pub fn texture_for_tileset(&self, relative_path: &str) -> Option<&Texture2D> {
        self.textures.get(relative_path)
    }

    pub fn entity_instances(&self, z: i32) -> Vec<LdtkDrawEntity> {
        let mut instances = Vec::new();
        for (level_index, level) in self.project.levels.iter().enumerate() {
            if level.world_depth != z {
                continue;
            }
            if let Some(layers) = &level.layer_instances {
                for (layer_index, layer) in layers.iter().enumerate() {
                    if !layer.visible {
                        continue;
                    }
                    for (entity_index, entity) in layer.entity_instances.iter().enumerate() {
                        instances.push(LdtkDrawEntity {
                            level_index,
                            layer_index,
                            entity_index,
                            world_x: (level.world_x + layer.offset_x) as f32
                                / GRID_PIXELS_PER_METER
                                + entity.px[0] / GRID_PIXELS_PER_METER,
                            world_y: (level.world_y + layer.offset_y) as f32
                                / GRID_PIXELS_PER_METER
                                + entity.px[1] / GRID_PIXELS_PER_METER,
                        });
                    }
                }
            }
        }
        instances
    }

    pub fn player_spawn(&self) -> Vec2 {
        for level in &self.project.levels {
            if let Some(layers) = &level.layer_instances {
                for layer in layers {
                    if !layer.visible {
                        continue;
                    }
                    if let Some(entity) = layer
                        .entity_instances
                        .iter()
                        .find(|entity| entity.identifier == "PlayerSpawn")
                    {
                        return Vec2::new(
                            (level.world_x + layer.offset_x) as f32 / GRID_PIXELS_PER_METER
                                + entity.px[0] / GRID_PIXELS_PER_METER,
                            (level.world_y + layer.offset_y) as f32 / GRID_PIXELS_PER_METER
                                + entity.px[1] / GRID_PIXELS_PER_METER,
                        );
                    }
                }
            }
        }
        let level = &self.project.levels[0];
        Vec2::new(
            (level.world_x as f32 + level.px_wid as f32 * 0.5) / GRID_PIXELS_PER_METER,
            (level.world_y as f32 + level.px_hei as f32 * 0.5) / GRID_PIXELS_PER_METER,
        )
    }

    pub fn collision_world(&self) -> LdtkCollisionWorld {
        LdtkCollisionWorld {
            project: self.project.clone(),
            entity_packs: self.entity_packs.clone(),
        }
    }

    pub fn is_solid_world_pos(&self, x: f32, y: f32, z: i32) -> bool {
        self.collision_world().is_solid_world_pos(x, y, z)
    }

    fn warn_missing_texture(&mut self, path: &str, error: &str) {
        if self.warned_missing.insert(path.to_string()) {
            eprintln!("Warning: unable to load LDtk sprite or tileset {path}: {error}");
        }
    }

    async fn load_texture(
        &mut self,
        relative_path: &str,
        relative_to_world: bool,
    ) -> Result<(), String> {
        if self.textures.contains_key(relative_path) {
            return Ok(());
        }
        let path = if relative_to_world {
            self.world_dir.join(relative_path)
        } else {
            PathBuf::from(relative_path)
        };
        let load_path = path.to_string_lossy().replace('\\', "/");
        let bytes = match load_file(&load_path).await {
            Ok(bytes) => bytes,
            Err(error) => {
                self.warn_missing_texture(&load_path, &error.to_string());
                return Ok(());
            }
        };
        image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
            .map_err(|error| format!("Failed decoding texture {load_path}: {error}"))?;
        let texture = Texture2D::from_file_with_format(&bytes, Some(ImageFormat::Png));
        texture.set_filter(FilterMode::Nearest);
        self.textures.insert(relative_path.to_string(), texture);
        Ok(())
    }
}
