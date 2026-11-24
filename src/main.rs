#![allow(dead_code)]

mod audio;
mod config;
mod dialogue;
mod entities;
mod render;
mod sim;
mod world;

use audio::AudioBank;
use config::UniverseConfig;
use entities::{CommandPacket, DeltaLedger, PlayerId};
use macroquad::prelude::*;
use render::{draw_hud_and_forge, draw_world, PaintedAtlas};
use sim::Universe;
use world::Location;

fn window_conf() -> Conf {
    Conf {
        window_title: "Clachan".to_owned(),
        window_width: 1280,
        window_height: 720,
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let atlas = PaintedAtlas::build();
    let sfx = AudioBank::synthesize().await;
    let mut universe = Universe::new(UniverseConfig::preset_1744_lochaber());
    let mut forge_open = false;
    let mut zoom_m = 22.0f32;

    let mut rt_w = screen_width().max(64.0) as u32;
    let mut rt_h = screen_height().max(64.0) as u32;
    let mut rt = render_target(rt_w, rt_h);
    rt.texture.set_filter(FilterMode::Linear);

    let mut cam = Vec2::new(-8.5, 0.5);
    let mut prev_loc = Location::Overworld;
    let mut accum = 0.0f32;
    let fixed_dt = 1.0 / 60.0;

    loop {
        let sw = screen_width().max(64.0);
        let sh = screen_height().max(64.0);
        if sw as u32 != rt_w || sh as u32 != rt_h {
            rt_w = sw as u32;
            rt_h = sh as u32;
            rt = render_target(rt_w, rt_h);
            rt.texture.set_filter(FilterMode::Linear);
        }

        let (_, wy) = mouse_wheel();
        if wy.abs() > 0.01 {
            zoom_m = (zoom_m - wy * 1.5).clamp(12.0, 38.0);
        }

        if is_key_pressed(KeyCode::Tab) {
            forge_open = !forge_open;
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
                    universe = Universe::new(cfg);
                    universe.apply_ledger(l);
                    universe.toast("Delta Ledger restored.");
                }
                Err(e) => universe.toast(format!("Load error: {}", e)),
            }
        }

        let cur_loc = universe.player_location(PlayerId(1));
        let aspect = sw / sh;
        // Zoom in warmly when inside a separated building interior
        let view_h_m = match cur_loc {
            Location::Overworld => zoom_m,
            Location::Interior(_) => 13.0,
        };
        let view_w_m = view_h_m * aspect;

        let mut dir = Vec2::ZERO;
        if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) { dir.y -= 1.0; }
        if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) { dir.y += 1.0; }
        if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) { dir.x -= 1.0; }
        if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) { dir.x += 1.0; }
        let sprint = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);

        let (mx, my) = mouse_position();
        let mouse_world = Vec2::new(
            cam.x + (mx / sw - 0.5) * view_w_m,
            cam.y + (my / sh - 0.5) * view_h_m,
        );
        let ppos = universe
            .player_index(PlayerId(1))
            .map(|i| universe.entities[i].pos)
            .unwrap_or(Vec2::ZERO);
        let aim = (mouse_world - ppos).normalize_or_zero();

        universe.apply_command(CommandPacket::Move { player: PlayerId(1), dir, sprint }, &sfx);

        let click_in_world =
            is_mouse_button_pressed(MouseButton::Left) && (!forge_open || mx < sw - 335.0);
        if click_in_world || is_key_pressed(KeyCode::Space) {
            universe.apply_command(CommandPacket::SwingSteel { player: PlayerId(1), aim }, &sfx);
        }
        if is_key_pressed(KeyCode::F) {
            universe.apply_command(CommandPacket::FireFlintlock { player: PlayerId(1), aim }, &sfx);
        }
        if is_key_pressed(KeyCode::E) {
            universe.apply_command(CommandPacket::Interact { player: PlayerId(1) }, &sfx);
        }
        if is_key_pressed(KeyCode::B) {
            universe.apply_command(CommandPacket::BuryCarried { player: PlayerId(1) }, &sfx);
        }
        if is_key_pressed(KeyCode::H) {
            universe.apply_command(CommandPacket::WhistleMount { player: PlayerId(1) }, &sfx);
        }
        if is_key_pressed(KeyCode::R) {
            universe.apply_command(CommandPacket::BindWounds { player: PlayerId(1) }, &sfx);
        }
        if is_key_pressed(KeyCode::V) {
            universe.apply_command(CommandPacket::WhetBlade { player: PlayerId(1) }, &sfx);
        }
        if is_key_pressed(KeyCode::C) {
            universe.apply_command(CommandPacket::BuildCampfire { player: PlayerId(1) }, &sfx);
        }

        accum = (accum + get_frame_time().min(0.08)).min(0.16);
        while accum >= fixed_dt {
            universe.step(fixed_dt, &sfx);
            accum -= fixed_dt;
        }

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
            cam = cam.lerp(cam_target, (get_frame_time() * 8.0).min(1.0));
        }

        // Pass 1: Render Overworld OR Separated Interior into offscreen RenderTarget
        set_camera(&Camera2D {
            target: cam,
            zoom: vec2(2.0 / view_w_m, 2.0 / view_h_m),
            render_target: Some(rt.clone()),
            ..Default::default()
        });
        let bg = match active_loc {
            Location::Overworld => Color::new(0.14, 0.24, 0.28, 1.0),
            Location::Interior(_) => Color::new(0.04, 0.04, 0.05, 1.0),
        };
        clear_background(bg);
        draw_world(&universe, &atlas, cam, view_w_m, view_h_m);

        // Pass 2: Oil-Painting & Atmosphere Shader (cozy dry warmth indoors, Highland mist outdoors)
        set_default_camera();
        clear_background(BLACK);
        let (warmth_u, mist_u, dark_u) = match active_loc {
            Location::Overworld => (
                universe.weather.sun_warmth,
                universe.weather.mist_intensity,
                universe.cfg.era_darkness,
            ),
            Location::Interior(bid) => {
                let lit = universe
                    .building_by_id(bid)
                    .map(|b| b.hearth_lit)
                    .unwrap_or(true);
                (if lit { 0.92 } else { 0.42 }, 0.0, universe.cfg.era_darkness * 0.35)
            }
        };
        atlas.post_mat.set_uniform("u_time", universe.time);
        atlas.post_mat.set_uniform("u_warmth", warmth_u);
        atlas.post_mat.set_uniform("u_darkness", dark_u);
        atlas.post_mat.set_uniform("u_mist", mist_u);
        atlas.post_mat.set_uniform("u_res_x", sw);
        atlas.post_mat.set_uniform("u_res_y", sh);

        gl_use_material(&atlas.post_mat);
        draw_texture_ex(
            &rt.texture,
            0.0,
            0.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(sw, sh)),
                ..Default::default()
            },
        );
        gl_use_default_material();

        // Pass 3: HUD, Fade Curtain & Live Universe Forge
        draw_hud_and_forge(&mut universe, &mut forge_open);

        next_frame().await;
    }
}
