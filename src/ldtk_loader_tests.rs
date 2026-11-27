use super::*;
use std::fs;

#[test]
fn starter_project_loads_and_player_spawn_is_found() {
    let text = fs::read_to_string(PROJECT_PATH).expect("starter LDtk project exists");
    let project: LdtkProject = serde_json::from_str(&text).expect("starter LDtk JSON deserializes");
    assert_eq!(project.levels.len(), 1);
    assert_eq!(project.default_grid_size, 32);
    assert_eq!(
        TILE_SIZE_PIXELS as f32 / PIXELS_PER_METER,
        crate::world::TILE_SIZE_M
    );
    assert_eq!(project.levels[0].identifier, "Lochaber_Glen");
    assert_eq!(
        project.levels[0]
            .layer_instances
            .as_ref()
            .expect("embedded layer instances")
            .iter()
            .map(|layer| layer.identifier.as_str())
            .collect::<Vec<_>>(),
        ["Overhead", "Entities", "Collision", "Ground"]
    );
    assert_eq!(project.levels[0].px_wid, 2048);
    assert_eq!(project.levels[0].px_hei, 2048);
    assert!(project.levels[0]
        .layer_instances
        .as_ref()
        .unwrap()
        .iter()
        .all(|layer| layer.grid_size == 32));
    let spawn = project.levels[0]
        .layer_instances
        .as_ref()
        .unwrap()
        .iter()
        .flat_map(|layer| &layer.entity_instances)
        .find(|entity| entity.identifier == "PlayerSpawn")
        .expect("project contains the player spawn");
    assert_eq!(spawn.px, [1024.0, 1024.0]);

    let loader = LdtkLoader {
        project,
        entity_packs: HashMap::new(),
        textures: HashMap::new(),
        world_dir: PathBuf::from("assets/world"),
        warned_missing: HashSet::new(),
    };
    assert_eq!(loader.player_spawn(), Vec2::new(32.0, 32.0));
}

#[test]
fn collision_uses_positive_intgrid_values_and_entity_colliders_at_z() {
    let project: LdtkProject = serde_json::from_value(serde_json::json!({
        "levels": [{
            "identifier": "test",
            "pxWid": 64,
            "pxHei": 64,
            "worldX": 0,
            "worldY": 0,
            "worldDepth": 0,
            "layerInstances": [{
                "__identifier": "Collision",
                "__type": "IntGrid",
                "__cWid": 2,
                "__cHei": 2,
                "__gridSize": 32,
                "intGridCsv": [0, 1, 0, 0],
                "entityInstances": [{
                    "__identifier": "Rock",
                    "iid": "rock",
                    "px": [0, 32],
                    "width": 32,
                    "height": 32,
                    "__pivot": [0.5, 0.5],
                    "fieldInstances": []
                }]
            }]
        }]
    }))
    .expect("collision fixture deserializes");
    let mut entity_packs = HashMap::new();
    entity_packs.insert(
        "Rock".to_string(),
        EntityPack {
            sprite_path: "rock.png".to_string(),
            hooded_sprite_path: None,
            kilt_layer_path: None,
            cloak_layer_path: None,
            hooded_cloak_layer_path: None,
            frame_size: (16, 16),
            foot_offset_y: 1.0,
            collider: Some((0.0, 0.0, 1.0, 1.0)),
        },
    );
    let world = LdtkCollisionWorld {
        project,
        entity_packs,
    };

    assert!(world.is_solid_world_pos(1.5, 0.5, 0));
    assert!(world.is_solid_world_pos(0.5, 1.5, 0));
    assert!(!world.is_solid_world_pos(1.5, 0.5, 1));
    assert!(!world.is_solid_world_pos(0.5, 0.5, 0));
}

#[test]
fn shelter_comes_from_shelter_intgrid_or_overhead_tiles_at_matching_depth() {
    let project: LdtkProject = serde_json::from_value(serde_json::json!({
        "levels": [{
            "identifier": "test",
            "pxWid": 64,
            "pxHei": 32,
            "worldDepth": 0,
            "layerInstances": [
                {
                    "__identifier": "Overhead_pines",
                    "__type": "Tiles",
                    "__gridSize": 32,
                    "gridTiles": [{"px": [32, 0], "src": [0, 0], "f": 0}]
                },
                {
                    "__identifier": "Shelter",
                    "__type": "IntGrid",
                    "__cWid": 2,
                    "__cHei": 1,
                    "__gridSize": 32,
                    "intGridCsv": [1, 0]
                }
            ]
        }]
    }))
    .expect("shelter fixture deserializes");
    let world = LdtkCollisionWorld {
        project,
        entity_packs: HashMap::new(),
    };

    assert!(world.is_sheltered_world_pos(0.5, 0.5, 0));
    assert!(world.is_sheltered_world_pos(1.5, 0.5, 0));
    assert!(!world.is_sheltered_world_pos(1.5, 0.5, 1));
    assert!(!world.is_sheltered_world_pos(0.5, 0.5, 1));
}

#[test]
fn entity_pack_without_hooded_sprite_still_deserializes() {
    let pack: EntityPack = ron::from_str(
        r#"(sprite_path:"assets/sprites/test.png",frame_size:(16,16),foot_offset_y:1.0,collider:None)"#,
    )
    .expect("hooded sprite is optional for existing packs");
    assert_eq!(pack.hooded_sprite_path, None);
    assert_eq!(pack.kilt_layer_path, None);
    assert_eq!(pack.cloak_layer_path, None);
    assert_eq!(pack.hooded_cloak_layer_path, None);
}

#[test]
fn entity_pack_deserializes_optional_paper_doll_layers() {
    let pack: EntityPack = ron::from_str(
        r#"(sprite_path:"assets/sprites/base.png",kilt_layer_path:Some("assets/sprites/kilt.png"),cloak_layer_path:Some("assets/sprites/cloak.png"),hooded_cloak_layer_path:Some("assets/sprites/hooded_cloak.png"),frame_size:(16,32),foot_offset_y:1.8,collider:None)"#,
    )
    .expect("paper doll fields deserialize");
    assert_eq!(
        pack.kilt_layer_path.as_deref(),
        Some("assets/sprites/kilt.png")
    );
    assert_eq!(
        pack.cloak_layer_path.as_deref(),
        Some("assets/sprites/cloak.png")
    );
    assert_eq!(
        pack.hooded_cloak_layer_path.as_deref(),
        Some("assets/sprites/hooded_cloak.png")
    );
    assert_eq!(pack.hooded_sprite_path, None);
}

#[test]
fn base_entity_frames_match_the_hibit_pixel_scale() {
    let text = fs::read_to_string(ENTITY_PACK_PATH).expect("base entity pack exists");
    let packs: HashMap<String, EntityPack> =
        ron::from_str(&text).expect("base entity pack deserializes");

    assert_eq!(
        packs["Highlander"].sprite_path,
        "assets/sprites/warden_base.png"
    );
    assert_eq!(packs["Highlander"].frame_size, (64, 64));
    assert_eq!(packs["Highlander"].foot_offset_y, 1.9);
    assert_eq!(packs["ScotsPine"].frame_size, (64, 96));
    assert_eq!(packs["Rock"].frame_size, (32, 64));
    assert_eq!(packs["Campfire"].frame_size, (32, 32));
    assert_eq!(packs["Hearth"].frame_size, (32, 32));
}

#[test]
fn light_markers_resolve_entity_positions_and_filter_by_depth() {
    let project: LdtkProject = serde_json::from_value(serde_json::json!({
        "levels": [{
            "identifier": "test",
            "worldX": 32,
            "worldY": 64,
            "pxWid": 128,
            "pxHei": 128,
            "worldDepth": 2,
            "layerInstances": [{
                "__identifier": "Entities",
                "__type": "Entities",
                "__pxTotalOffsetX": 32,
                "__pxTotalOffsetY": 0,
                "entityInstances": [{
                    "__identifier": "CanopyGap",
                    "iid": "gap",
                    "px": [32, 16],
                    "width": 64,
                    "height": 32,
                    "__pivot": [0.5, 0.5],
                    "fieldInstances": []
                }]
            }]
        }]
    }))
    .expect("light marker fixture deserializes");
    let loader = LdtkLoader {
        project,
        entity_packs: HashMap::new(),
        textures: HashMap::new(),
        world_dir: PathBuf::from("assets/world"),
        warned_missing: HashSet::new(),
    };

    assert_eq!(
        loader.light_markers(2, "CanopyGap"),
        vec![(Vec2::new(3.0, 2.5), 2.5)]
    );
    assert!(loader.light_markers(1, "CanopyGap").is_empty());
}

#[test]
fn missing_sprite_warning_is_deduplicated() {
    let mut loader = LdtkLoader {
        project: LdtkProject {
            default_grid_size: 32,
            levels: Vec::new(),
        },
        entity_packs: HashMap::new(),
        textures: HashMap::new(),
        world_dir: PathBuf::from("assets/world"),
        warned_missing: HashSet::new(),
    };
    loader.warn_missing_texture("assets/no-such-sprite.png", "not found");
    loader.warn_missing_texture("assets/no-such-sprite.png", "not found");
    assert_eq!(loader.warned_missing.len(), 1);
}
