#[cfg(not(target_arch = "wasm32"))]
mod inner {
    use crate::world::Rng;
    use macroquad::audio::{load_sound_from_bytes, play_sound, PlaySoundParams, Sound};

    pub type SfxSlot = Option<Sound>;

    pub struct AudioBank {
        pub steel: SfxSlot,
        pub rope_snap: SfxSlot,
        pub gunshot: SfxSlot,
        pub misfire: SfxSlot,
        pub whistle: SfxSlot,
        pub voice_bark: SfxSlot,
    }

    fn build_wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
        let data_len = (samples.len() * 2) as u32;
        let mut buf = Vec::with_capacity(44 + data_len as usize);
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&(36 + data_len).to_le_bytes());
        buf.extend_from_slice(b"WAVE");
        buf.extend_from_slice(b"fmt ");
        buf.extend_from_slice(&16u32.to_le_bytes());
        buf.extend_from_slice(&1u16.to_le_bytes());
        buf.extend_from_slice(&1u16.to_le_bytes());
        buf.extend_from_slice(&sample_rate.to_le_bytes());
        buf.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        buf.extend_from_slice(&2u16.to_le_bytes());
        buf.extend_from_slice(&16u16.to_le_bytes());
        buf.extend_from_slice(b"data");
        buf.extend_from_slice(&data_len.to_le_bytes());
        for &s in samples {
            let v = (s.clamp(-1.0, 1.0) * 32760.0) as i16;
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf
    }

    impl AudioBank {
        pub async fn synthesize() -> Self {
            let sr = 22050u32;
            let mut rng = Rng::new(0x5049_4E45);

            let steel_s: Vec<f32> = (0..(sr / 5))
                .map(|i| {
                    let t = i as f32 / sr as f32;
                    let env = (-t * 18.0).exp();
                    let ring = (t * 2400.0 * std::f32::consts::TAU).sin() * 0.25
                        + (t * 3650.0 * std::f32::consts::TAU).sin() * 0.15;
                    let air = (rng.next_f32() * 2.0 - 1.0) * 0.45;
                    (ring + air) * env * 0.5
                })
                .collect();

            let rope_s: Vec<f32> = (0..(sr / 6))
                .map(|i| {
                    let t = i as f32 / sr as f32;
                    let env = (-t * 34.0).exp();
                    let crack = (rng.next_f32() * 2.0 - 1.0) * 0.7;
                    let twang =
                        (t * (420.0 - t * 1400.0).max(80.0) * std::f32::consts::TAU).sin() * 0.5;
                    (crack + twang) * env * 0.6
                })
                .collect();

            let mut lp = 0.0f32;
            let gun_s: Vec<f32> = (0..(sr / 2))
                .map(|i| {
                    let t = i as f32 / sr as f32;
                    let env = (-t * 6.5).exp();
                    let white = rng.next_f32() * 2.0 - 1.0;
                    lp = lp * 0.78 + white * 0.22;
                    let boom = (t * (110.0 * (-t * 4.0).exp()) * std::f32::consts::TAU).sin() * 0.7;
                    (lp * 1.4 + boom) * env * 0.75
                })
                .collect();

            let misfire_s: Vec<f32> = (0..(sr / 5))
                .map(|i| {
                    let t = i as f32 / sr as f32;
                    let click = if t < 0.012 {
                        (t * 4800.0 * std::f32::consts::TAU).sin() * 0.6
                    } else {
                        0.0
                    };
                    let fizz = (rng.next_f32() * 2.0 - 1.0) * (-t * 14.0).exp() * 0.22;
                    click + fizz
                })
                .collect();

            let whistle_s: Vec<f32> = (0..(sr / 3))
                .map(|i| {
                    let t = i as f32 / sr as f32;
                    let env = (t * 15.0).min(1.0) * (-t * 4.5).exp();
                    let freq = 1150.0 + (t * 16.0).sin() * 140.0 + t * 480.0;
                    (t * freq * std::f32::consts::TAU).sin() * env * 0.35
                })
                .collect();

            let bark_s: Vec<f32> = (0..(sr / 5))
                .map(|i| {
                    let t = i as f32 / sr as f32;
                    let env = (t * 25.0).min(1.0) * (-t * 11.0).exp();
                    let f0 = 132.0 - t * 22.0;
                    let glottal = ((t * f0) % 1.0) * 2.0 - 1.0;
                    let f1 = (t * 560.0 * std::f32::consts::TAU).sin();
                    let f2 = (t * 1420.0 * std::f32::consts::TAU).sin() * 0.5;
                    (glottal * 0.35 + f1 * 0.4 + f2 * 0.25) * env * 0.4
                })
                .collect();

            Self {
                steel: load_sound_from_bytes(&build_wav(&steel_s, sr)).await.ok(),
                rope_snap: load_sound_from_bytes(&build_wav(&rope_s, sr)).await.ok(),
                gunshot: load_sound_from_bytes(&build_wav(&gun_s, sr)).await.ok(),
                misfire: load_sound_from_bytes(&build_wav(&misfire_s, sr)).await.ok(),
                whistle: load_sound_from_bytes(&build_wav(&whistle_s, sr)).await.ok(),
                voice_bark: load_sound_from_bytes(&build_wav(&bark_s, sr)).await.ok(),
            }
        }

        pub fn play(sound: &SfxSlot, vol: f32) {
            if let Some(s) = sound {
                play_sound(
                    s,
                    PlaySoundParams {
                        looped: false,
                        volume: vol,
                    },
                );
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod inner {
    pub type SfxSlot = Option<()>;

    pub struct AudioBank {
        pub steel: SfxSlot,
        pub rope_snap: SfxSlot,
        pub gunshot: SfxSlot,
        pub misfire: SfxSlot,
        pub whistle: SfxSlot,
        pub voice_bark: SfxSlot,
    }

    impl AudioBank {
        pub async fn synthesize() -> Self {
            Self {
                steel: None,
                rope_snap: None,
                gunshot: None,
                misfire: None,
                whistle: None,
                voice_bark: None,
            }
        }

        pub fn play(_sound: &SfxSlot, _vol: f32) {}
    }
}

pub use inner::AudioBank;
