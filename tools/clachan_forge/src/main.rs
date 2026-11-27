mod world_gen;

use image::{Rgba, RgbaImage};
use std::env;
use std::fs;

#[derive(Clone, Copy)]
struct Swatch {
    rgb: [u8; 3],
    lab: [f32; 3],
}

struct Palette {
    swatches: Vec<Swatch>,
}

impl Palette {
    fn from_gpl(path: &str) -> Result<Self, String> {
        let text =
            fs::read_to_string(path).map_err(|error| format!("Failed reading {path}: {error}"))?;
        let swatches = text
            .lines()
            .filter_map(|line| {
                let mut parts = line.split_whitespace();
                let r = parts.next()?.parse::<u8>().ok()?;
                let g = parts.next()?.parse::<u8>().ok()?;
                let b = parts.next()?.parse::<u8>().ok()?;
                Some(Swatch {
                    rgb: [r, g, b],
                    lab: rgb_to_lab([r, g, b]),
                })
            })
            .collect::<Vec<_>>();
        if swatches.is_empty() {
            return Err("No valid RGB entries found in palette file.".into());
        }
        Ok(Self { swatches })
    }

    fn nearest(&self, rgb: [u8; 3]) -> [u8; 3] {
        let target = rgb_to_lab(rgb);
        self.swatches
            .iter()
            .min_by(|left, right| {
                lab_distance(target, left.lab).total_cmp(&lab_distance(target, right.lab))
            })
            .map(|swatch| swatch.rgb)
            .unwrap_or(rgb)
    }
}

fn lab_distance(left: [f32; 3], right: [f32; 3]) -> f32 {
    (left[0] - right[0]).powi(2) + (left[1] - right[1]).powi(2) + (left[2] - right[2]).powi(2)
}

fn rgb_to_lab(rgb: [u8; 3]) -> [f32; 3] {
    let to_lin = |channel: u8| {
        let value = channel as f32 / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = (to_lin(rgb[0]), to_lin(rgb[1]), to_lin(rgb[2]));
    let x = (r * 0.4124 + g * 0.3576 + b * 0.1805) / 0.95047;
    let y = r * 0.2126 + g * 0.7152 + b * 0.0722;
    let z = (r * 0.0193 + g * 0.1192 + b * 0.9505) / 1.08883;
    let f = |value: f32| {
        if value > 0.008856 {
            value.cbrt()
        } else {
            7.787 * value + 16.0 / 116.0
        }
    };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

fn flash_sprites(
    palette: &Palette,
    inputs: &[String],
    output: &str,
    dither_strength: f32,
) -> Result<(), String> {
    if inputs.is_empty() {
        return Err("No input PNG files provided to flash.".into());
    }
    let bayer = [[0.0f32, 0.5], [0.75, 0.25]];
    let outline = palette.nearest([18, 14, 28]);
    let mut frames = Vec::with_capacity(inputs.len());

    for path in inputs {
        let raw = image::open(path)
            .map_err(|error| format!("Failed opening {path}: {error}"))?
            .to_rgba8();
        let (width, height) = raw.dimensions();
        let mut snapped = RgbaImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let pixel = raw.get_pixel(x, y);
                if pixel[3] < 128 {
                    continue;
                }
                let bias = (bayer[(y & 1) as usize][(x & 1) as usize] - 0.375) * dither_strength;
                let rgb = [
                    (pixel[0] as f32 + bias).clamp(0.0, 255.0) as u8,
                    (pixel[1] as f32 + bias).clamp(0.0, 255.0) as u8,
                    (pixel[2] as f32 + bias).clamp(0.0, 255.0) as u8,
                ];
                let color = palette.nearest(rgb);
                snapped.put_pixel(x, y, Rgba([color[0], color[1], color[2], 255]));
            }
        }

        let mut outlined = snapped.clone();
        for y in 0..height {
            for x in 0..width {
                if snapped.get_pixel(x, y)[3] != 0 {
                    continue;
                }
                let touches_solid = (x > 0 && snapped.get_pixel(x - 1, y)[3] != 0)
                    || (x + 1 < width && snapped.get_pixel(x + 1, y)[3] != 0)
                    || (y > 0 && snapped.get_pixel(x, y - 1)[3] != 0)
                    || (y + 1 < height && snapped.get_pixel(x, y + 1)[3] != 0);
                if touches_solid {
                    outlined.put_pixel(x, y, Rgba([outline[0], outline[1], outline[2], 255]));
                }
            }
        }
        frames.push(outlined);
    }

    let (frame_width, frame_height) = frames[0].dimensions();
    if frames
        .iter()
        .any(|frame| frame.dimensions() != (frame_width, frame_height))
    {
        return Err("All frames must have identical dimensions.".into());
    }
    let mut sheet = RgbaImage::new(frame_width * frames.len() as u32, frame_height);
    for (index, frame) in frames.iter().enumerate() {
        for y in 0..frame_height {
            for x in 0..frame_width {
                sheet.put_pixel(index as u32 * frame_width + x, y, *frame.get_pixel(x, y));
            }
        }
    }
    sheet
        .save(output)
        .map_err(|error| format!("Failed saving {output}: {error}"))?;
    println!("Flashed {} frame(s) -> {output}", frames.len());
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage:");
        eprintln!("  clachan_forge flash <palette.gpl> <out.png> [--dither N] <frame1.png> ...");
        eprintln!("  clachan_forge generate-world [out_dir] [seed]");
        std::process::exit(1);
    }

    let result = match args[1].as_str() {
        "generate-world" => {
            let out_dir = args.get(2).map(String::as_str).unwrap_or("assets/world");
            let seed = args
                .get(3)
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(1744);
            world_gen::generate_world(out_dir, seed)
        }
        "flash" if args.len() >= 5 => {
            let mut dither = 10.0f32;
            let mut inputs = Vec::new();
            let mut index = 4;
            while index < args.len() {
                if args[index] == "--dither" && index + 1 < args.len() {
                    dither = args[index + 1].parse().unwrap_or(10.0);
                    index += 2;
                } else {
                    inputs.push(args[index].clone());
                    index += 1;
                }
            }
            Palette::from_gpl(&args[2])
                .and_then(|palette| flash_sprites(&palette, &inputs, &args[3], dither))
        }
        "flash" => {
            Err("Expected: flash <palette.gpl> <out.png> [--dither N] <frame1.png> ...".into())
        }
        other => Err(format!("Unknown command: {other}")),
    };

    if let Err(error) = result {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
