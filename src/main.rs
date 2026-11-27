#![allow(dead_code)]

mod assets;
mod audio;
mod config;
mod dialogue;
mod entities;
mod ldtk_loader;
mod render;
mod render_sort;
mod sim;
mod world;
mod world_stream;

use assets::AssetBank;
use audio::AudioBank;
use chrono::{Local, Timelike};
use config::{UniverseConfig, PIXELS_PER_METER, VIRTUAL_HEIGHT, VIRTUAL_WIDTH};
use entities::{CommandPacket, DeltaLedger, PlayerId};
use ldtk_loader::LdtkLoader;
use macroquad::prelude::*;
use render::{
    draw_atmosphere_pass, draw_ground_pass, draw_lightmap_pass, draw_overhead_pass,
    draw_traces_pass, draw_ui_pass, draw_y_sorted_pass, load_multiply_material, RegionAtmosphere,
};
use sim::Universe;
use std::sync::Arc;
use world::{Location, WeatherMode};
use world_stream::WorldBlueprint;

const VIEW_WIDTH_M: f32 = VIRTUAL_WIDTH as f32 / PIXELS_PER_METER;
const VIEW_HEIGHT_M: f32 = VIRTUAL_HEIGHT as f32 / PIXELS_PER_METER;

fn window_conf() -> Conf {
    Conf {
        window_title: "An Tìr Shàmhach".to_owned(),
        window_width: 1280,
        window_height: 720,
        high_dpi: true,
        ..Default::default()
    }
}

fn place_entities_at_ldtk_spawn(universe: &mut Universe, spawn: Vec2) {
    let Some(player_index) = universe.player_index(PlayerId(1)) else {
        return;
    };
    let offset = spawn - universe.entities[player_index].pos;
    for entity in &mut universe.entities {
        entity.pos += offset;
        if let Some(mind) = &mut entity.mind {
            mind.home_pos += offset;
        }
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let blueprint = match WorldBlueprint::load() {
        Ok(blueprint) => Arc::new(blueprint),
        Err(error) => {
            eprintln!("World blueprint loading failed: {}", error);
            return;
        }
    };
    let mut assets = match AssetBank::load().await {
        Ok(assets) => assets,
        Err(error) => {
            eprintln!("Asset loading failed: {}", error);
            return;
        }
    };
    #[cfg_attr(
        not(all(not(target_arch = "wasm32"), debug_assertions)),
        allow(unused_mut)
    )]
    let mut ldtk = match LdtkLoader::load().await {
        Ok(ldtk) => ldtk,
        Err(error) => {
            eprintln!("LDtk world loading failed: {error}");
            return;
        }
    };
    let sfx = AudioBank::synthesize().await;
    let region = RegionAtmosphere::load_starting_region().await;
    let mut universe = Universe::new(UniverseConfig::pine_edge(), blueprint.clone());
    let spawn = ldtk.player_spawn();
    place_entities_at_ldtk_spawn(&mut universe, spawn);
    universe.set_ldtk_collision_world(Arc::new(ldtk.collision_world()));
    let mut world_map_open = false;
    let mut debug_view = false;
    let mut time_preview = 0usize;
    let rt = render_target(VIRTUAL_WIDTH, VIRTUAL_HEIGHT);
    let light_rt = render_target(VIRTUAL_WIDTH, VIRTUAL_HEIGHT);
    let ui_rt = render_target(VIRTUAL_WIDTH, VIRTUAL_HEIGHT);
    rt.texture.set_filter(FilterMode::Nearest);
    light_rt.texture.set_filter(FilterMode::Nearest);
    ui_rt.texture.set_filter(FilterMode::Nearest);
    let multiply_mat = load_multiply_material();

    let mut cam = spawn;
    let mut prev_loc = Location::Overworld;
    let mut accum = 0.0f32;
    let fixed_dt = 1.0 / 60.0;

    loop {
        let sw = screen_width().max(64.0);
        let sh = screen_height().max(64.0);
        let frame_dt = get_frame_time();
        let fit_scale = (sw / VIRTUAL_WIDTH as f32).min(sh / VIRTUAL_HEIGHT as f32);
        let output_scale = if fit_scale >= 1.0 {
            fit_scale.floor()
        } else {
            fit_scale
        };
        let output_w = VIRTUAL_WIDTH as f32 * output_scale;
        let output_h = VIRTUAL_HEIGHT as f32 * output_scale;
        let output_x = (sw - output_w) * 0.5;
        let output_y = (sh - output_h) * 0.5;

        #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
        let force_asset_reload = is_key_pressed(KeyCode::F6);
        #[cfg(not(all(not(target_arch = "wasm32"), debug_assertions)))]
        let force_asset_reload = false;
        match assets.poll_hot_reload(frame_dt, force_asset_reload).await {
            Ok(true) => {
                universe.toast("Reloaded UI textures.");
            }
            Ok(false) => {}
            Err(error) => universe.toast(format!("Asset reload failed: {}", error)),
        }
        #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
        if force_asset_reload {
            match ldtk.reload().await {
                Ok(()) => {
                    universe.set_ldtk_collision_world(Arc::new(ldtk.collision_world()));
                    universe.toast("Reloaded LDtk project and textures.");
                }
                Err(error) => universe.toast(format!("LDtk reload failed: {error}")),
            }
        }

        if is_key_pressed(KeyCode::M) {
            world_map_open = !world_map_open;
        }
        if is_key_pressed(KeyCode::F3) {
            debug_view = !debug_view;
        }
        if is_key_pressed(KeyCode::F4) {
            universe.weather.cycle_weather_mode();
            let message = match universe.weather.weather_mode {
                WeatherMode::Clear => "Weather test: Clear",
                WeatherMode::Mist => "Weather test: Mist",
                WeatherMode::Rain => "Weather test: Rain",
                WeatherMode::Snow => "Weather test: Snow",
                WeatherMode::Natural => "Weather test: Natural",
            };
            universe.toast(message);
        }
        if is_key_pressed(KeyCode::F7) {
            time_preview = (time_preview + 1) % 4;
            if time_preview == 0 {
                universe.weather.time_offset_hours = 0.0;
                universe.toast("Time preview: Real System Time");
            } else {
                let target_hour = match time_preview {
                    1 => 9.0,
                    2 => 18.5,
                    _ => 23.0,
                };
                let now = Local::now();
                let current_hour =
                    now.hour() as f32 + now.minute() as f32 / 60.0 + now.second() as f32 / 3600.0;
                universe.weather.time_offset_hours = target_hour - current_hour;
                let message = match time_preview {
                    1 => "Time preview: Morning",
                    2 => "Time preview: Golden Dusk",
                    _ => "Time preview: Moonlit Night",
                };
                universe.toast(message);
            }
        }
        if is_key_pressed(KeyCode::F5) {
            match universe.ledger.save_to_disk() {
                Ok(_) => universe.toast("Delta Ledger saved (clachan_save.ledger)."),
                Err(e) => universe.toast(format!("Save error: {}", e)),
            }
        }
        if is_key_pressed(KeyCode::F9) {
            match DeltaLedger::load_from_disk() {
                Ok(l) => {
                    let cfg = universe.cfg.clone();
                    universe = Universe::new(cfg, blueprint.clone());
                    place_entities_at_ldtk_spawn(&mut universe, ldtk.player_spawn());
                    universe.set_ldtk_collision_world(Arc::new(ldtk.collision_world()));
                    universe.apply_ledger(l);
                    universe.toast("Delta Ledger restored.");
                }
                Err(e) => universe.toast(format!("Load error: {}", e)),
            }
        }

        let view_w_m = VIEW_WIDTH_M;
        let view_h_m = VIEW_HEIGHT_M;

        let mut dir = Vec2::ZERO;
        if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) {
            dir.y -= 1.0;
        }
        if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) {
            dir.y += 1.0;
        }
        if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) {
            dir.x -= 1.0;
        }
        if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) {
            dir.x += 1.0;
        }
        let sprint = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);

        let (mouse_x, mouse_y) = mouse_position();
        let mx = (mouse_x - output_x) / output_scale;
        let my = (mouse_y - output_y) / output_scale;
        let hotbar_slot_w = 60.0;
        let hotbar_gap = 7.0;
        let hotbar_width = hotbar_slot_w * 5.0 + hotbar_gap * 4.0;
        let hotbar_x = (VIRTUAL_WIDTH as f32 - hotbar_width) * 0.5;
        let hotbar_y = VIRTUAL_HEIGHT as f32 - 56.0;
        let mouse_over_hotbar = mx >= hotbar_x
            && mx <= hotbar_x + hotbar_width
            && my >= hotbar_y
            && my <= hotbar_y + 54.0;
        let number_keys = [
            KeyCode::Key1,
            KeyCode::Key2,
            KeyCode::Key3,
            KeyCode::Key4,
            KeyCode::Key5,
        ];
        for (slot, key) in number_keys.iter().enumerate() {
            if is_key_pressed(*key) {
                universe.apply_command(
                    CommandPacket::SelectHotbar {
                        player: PlayerId(1),
                        slot,
                    },
                    &sfx,
                );
            }
        }
        if is_mouse_button_pressed(MouseButton::Left) && mouse_over_hotbar {
            let rel_x = mx - hotbar_x;
            if rel_x >= 0.0 && rel_x < hotbar_width {
                let slot = (rel_x / (hotbar_slot_w + hotbar_gap)).floor() as usize;
                let in_slot = rel_x % (hotbar_slot_w + hotbar_gap) <= hotbar_slot_w;
                if slot < 5 && in_slot {
                    universe.apply_command(
                        CommandPacket::SelectHotbar {
                            player: PlayerId(1),
                            slot,
                        },
                        &sfx,
                    );
                }
            }
        }
        let mouse_in_canvas = mouse_x >= output_x
            && mouse_x <= output_x + output_w
            && mouse_y >= output_y
            && mouse_y <= output_y + output_h;
        let mouse_world = Vec2::new(
            cam.x + ((mouse_x - output_x) / output_w - 0.5) * view_w_m,
            cam.y + ((mouse_y - output_y) / output_h - 0.5) * view_h_m,
        );
        let ppos = universe
            .player_index(PlayerId(1))
            .map(|i| universe.entities[i].pos)
            .unwrap_or(Vec2::ZERO);
        let aim = (mouse_world - ppos).normalize_or_zero();

        universe.apply_command(
            CommandPacket::Move {
                player: PlayerId(1),
                dir,
                sprint,
            },
            &sfx,
        );

        let click_in_world =
            is_mouse_button_pressed(MouseButton::Left) && !mouse_over_hotbar && mouse_in_canvas;
        if click_in_world {
            universe.apply_command(
                CommandPacket::UseHotbar {
                    player: PlayerId(1),
                    aim,
                },
                &sfx,
            );
        }
        if is_key_pressed(KeyCode::Space) {
            universe.apply_command(
                CommandPacket::SwingSteel {
                    player: PlayerId(1),
                    aim,
                },
                &sfx,
            );
        }
        if is_key_pressed(KeyCode::E) {
            universe.apply_command(
                CommandPacket::Interact {
                    player: PlayerId(1),
                },
                &sfx,
            );
        }
        if is_key_pressed(KeyCode::B) {
            universe.apply_command(
                CommandPacket::BuryCarried {
                    player: PlayerId(1),
                },
                &sfx,
            );
        }
        if is_key_pressed(KeyCode::H) {
            universe.apply_command(
                CommandPacket::WhistleMount {
                    player: PlayerId(1),
                },
                &sfx,
            );
        }
        if is_key_pressed(KeyCode::R) {
            universe.apply_command(
                CommandPacket::BindWounds {
                    player: PlayerId(1),
                },
                &sfx,
            );
        }
        if is_key_pressed(KeyCode::V) {
            universe.apply_command(
                CommandPacket::WhetBlade {
                    player: PlayerId(1),
                },
                &sfx,
            );
        }
        if is_key_pressed(KeyCode::C) {
            universe.apply_command(
                CommandPacket::BuildCampfire {
                    player: PlayerId(1),
                },
                &sfx,
            );
        }

        accum = (accum + frame_dt.min(0.08)).min(0.16);
        while accum >= fixed_dt {
            universe.step(fixed_dt, &sfx);
            accum -= fixed_dt;
        }
        universe.update_world_stream();

        let active_loc = universe.player_location(PlayerId(1));
        let cam_target = match active_loc {
            Location::Overworld => universe
                .player_index(PlayerId(1))
                .map(|i| universe.entities[i].pos)
                .unwrap_or(cam),
            Location::Interior(bid) => universe
                .building_by_id(bid)
                .map(|b| Vec2::new(b.int_w * 0.5, b.int_h * 0.5))
                .unwrap_or(cam),
        };
        if active_loc != prev_loc {
            cam = cam_target;
            prev_loc = active_loc;
        } else {
            cam = cam.lerp(cam_target, (frame_dt * 8.0).min(1.0));
        }
        cam = (cam * PIXELS_PER_METER).round() / PIXELS_PER_METER;

        // World, traces, sprites, and atmosphere render into the virtual scene target.
        set_camera(&Camera2D {
            target: cam,
            zoom: vec2(2.0 / view_w_m, 2.0 / view_h_m),
            render_target: Some(rt.clone()),
            ..Default::default()
        });
        clear_background(region.clear_color());
        let depth = universe
            .player_index(PlayerId(1))
            .map(|index| universe.entities[index].z.round() as i32)
            .unwrap_or_default();
        draw_ground_pass(
            &ldtk,
            &universe.world_stream,
            depth,
            cam,
            view_w_m,
            view_h_m,
            universe.time,
        );
        draw_traces_pass(&universe);
        draw_y_sorted_pass(&universe, &ldtk, depth, debug_view);
        draw_overhead_pass(&ldtk, depth);
        draw_atmosphere_pass(&universe, &region, cam, view_w_m, view_h_m);

        // Draw world-space lighting into its own target for multiplicative compositing.
        set_camera(&Camera2D {
            target: cam,
            zoom: vec2(2.0 / view_w_m, 2.0 / view_h_m),
            render_target: Some(light_rt.clone()),
            ..Default::default()
        });
        draw_lightmap_pass(&universe, &ldtk, &region, depth, cam, view_w_m, view_h_m);

        // Draw crisp, unlit UI at virtual resolution.
        set_camera(&Camera2D {
            target: vec2(VIRTUAL_WIDTH as f32 * 0.5, VIRTUAL_HEIGHT as f32 * 0.5),
            zoom: vec2(2.0 / VIRTUAL_WIDTH as f32, 2.0 / VIRTUAL_HEIGHT as f32),
            render_target: Some(ui_rt.clone()),
            ..Default::default()
        });
        clear_background(Color::new(0.0, 0.0, 0.0, 0.0));
        draw_ui_pass(&universe, &assets, world_map_open);

        // Composite the pixel-locked scene, multiplicative lightmap, and unlit UI.
        set_default_camera();
        clear_background(Color::from_rgba(14, 16, 15, 255));
        let screen_params = DrawTextureParams {
            dest_size: Some(vec2(output_w, output_h)),
            ..Default::default()
        };
        gl_use_default_material();
        draw_texture_ex(
            &rt.texture,
            output_x,
            output_y,
            WHITE,
            screen_params.clone(),
        );
        gl_use_material(&multiply_mat);
        draw_texture_ex(
            &light_rt.texture,
            output_x,
            output_y,
            WHITE,
            screen_params.clone(),
        );
        gl_use_default_material();
        draw_texture_ex(&ui_rt.texture, output_x, output_y, WHITE, screen_params);
        next_frame().await;
    }
}
