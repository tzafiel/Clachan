use image::{imageops::FilterType, GenericImage, Rgba, RgbaImage};
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::convert::TryFrom;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("ingest") => {
            if let Err(error) = run_ingest(&args[2..]) {
                eprintln!("Sprite ingest failed: {error}");
                std::process::exit(1);
            }
        }
        Some("pack-sheet") => {
            if let Err(error) = run_pack_sheet(&args[2..]) {
                eprintln!("Spritesheet packing failed: {error}");
                std::process::exit(1);
            }
        }
        _ => {
            println!(
            "Usage:\n  cargo run --bin clachan_forge -- ingest [--subdir <name>] [--snap-color R,G,B]\n  cargo run --bin clachan_forge -- pack-sheet <animation-folder> <output.png>"
        );
        }
    }
}

struct IngestOptions {
    subdir: Option<String>,
    snap_color: Option<[u8; 3]>,
}

fn parse_ingest_options(args: &[String]) -> Result<IngestOptions, String> {
    let mut options = IngestOptions {
        subdir: None,
        snap_color: None,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--subdir" => {
                let Some(name) = args.get(index + 1) else {
                    return Err("--subdir requires a folder name".into());
                };
                validate_subdir(name)?;
                options.subdir = Some(name.clone());
                index += 2;
            }
            "--snap-color" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("--snap-color requires R,G,B".into());
                };
                options.snap_color = Some(parse_rgb(value)?);
                index += 2;
            }
            other => return Err(format!("Unknown ingest option: {other}")),
        }
    }

    Ok(options)
}

fn validate_subdir(name: &str) -> Result<(), String> {
    if name.is_empty() || name == "." {
        return Err("--subdir must be a non-empty folder name".into());
    }
    let path = Path::new(name);
    if path.is_absolute()
        || path.components().count() != 1
        || matches!(
            path.components().next(),
            Some(std::path::Component::ParentDir)
        )
    {
        return Err("--subdir must name a single folder, not an absolute or parent path".into());
    }
    Ok(())
}

fn parse_rgb(value: &str) -> Result<[u8; 3], String> {
    let parts = value.split(',').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err("--snap-color must use R,G,B (for example 88,142,68)".into());
    }
    let channels = parts
        .iter()
        .map(|part| {
            part.parse::<u8>().map_err(|_| {
                "--snap-color channels must be integers from 0 through 255".to_string()
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok([channels[0], channels[1], channels[2]])
}

fn run_ingest(args: &[String]) -> Result<(), Box<dyn Error>> {
    let options = parse_ingest_options(args).map_err(std::io::Error::other)?;
    let raw_dir = Path::new("raw_art");
    let tiles_out = Path::new("assets/tilesets");
    let sprites_out = Path::new("assets/sprites");

    fs::create_dir_all(raw_dir)?;
    fs::create_dir_all(tiles_out)?;
    fs::create_dir_all(sprites_out)?;

    let tiles_out = match &options.subdir {
        Some(subdir) => tiles_out.join(subdir),
        None => tiles_out.to_path_buf(),
    };
    let sprites_out = match &options.subdir {
        Some(subdir) => sprites_out.join(subdir),
        None => sprites_out.to_path_buf(),
    };
    fs::create_dir_all(&tiles_out)?;
    fs::create_dir_all(&sprites_out)?;

    let mut count = 0;
    for entry in fs::read_dir(raw_dir)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };

        let Some((width, height)) = parse_dimensions(stem) else {
            println!("Skipping {:?} (add _WxH to filename, e.g. _48x48)", path);
            continue;
        };

        let image = match image::open(&path) {
            Ok(image) => image.to_rgba8(),
            Err(error) => {
                eprintln!("Failed to open {:?}: {}", path, error);
                continue;
            }
        };

        let mut output = resize_to_target(image, width, height);
        let is_sprite = stem.contains("_sprite");
        let is_tile = stem.contains("_tile");

        if is_sprite {
            strip_white_background(&mut output, 238);
        } else if is_tile {
            process_tile(&mut output, options.snap_color);
        }

        let clean_name = stem.replace("_sprite", "").replace("_tile", "");
        let destination_dir = if is_tile { &tiles_out } else { &sprites_out };
        let destination: PathBuf = destination_dir.join(format!("{clean_name}.png"));

        output.save(&destination)?;
        println!(
            "Ingested {:?} -> {:?} ({}x{})",
            path.file_name().unwrap_or_default(),
            destination,
            width,
            height
        );
        count += 1;
    }

    println!("Done! Processed {} file(s).", count);
    Ok(())
}

fn run_pack_sheet(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args.len() != 2 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "pack-sheet requires an animation folder and output PNG path",
        )
        .into());
    }

    let input_folder = Path::new(&args[0]);
    let output_path = Path::new(&args[1]);
    if !output_path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("png"))
        .unwrap_or(false)
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "pack-sheet output path must use a .png extension",
        )
        .into());
    }

    let packed = pack_animation_sheet(input_folder)?;
    if let Some(parent) = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    packed.image.save(output_path)?;
    println!(
        "Packed {}: sheet {}x{}, frame {}x{}, {} frames total ({} per direction)",
        output_path.display(),
        packed.image.width(),
        packed.image.height(),
        packed.frame_width,
        packed.frame_height,
        packed.frame_count * 4,
        packed.frame_count
    );
    Ok(())
}

struct PackedSheet {
    image: RgbaImage,
    frame_width: u32,
    frame_height: u32,
    frame_count: usize,
}

fn pack_animation_sheet(input_folder: &Path) -> Result<PackedSheet, Box<dyn Error>> {
    const DIRECTIONS: [&str; 4] = ["south", "north", "west", "east"];
    let mut rows = Vec::with_capacity(DIRECTIONS.len());
    let mut frame_size = None;
    let mut frame_count = None;

    for direction in DIRECTIONS {
        let directory = input_folder.join(direction);
        if !directory.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("missing direction folder: {}", directory.display()),
            )
            .into());
        }

        let mut paths = Vec::new();
        for entry in fs::read_dir(&directory)? {
            let path = entry?.path();
            if path.is_file()
                && path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .map(|extension| extension.eq_ignore_ascii_case("png"))
                    .unwrap_or(false)
            {
                paths.push(path);
            }
        }
        paths.sort_by(|left, right| natural_path_cmp(left, right));
        if paths.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("no PNG frames found in {}", directory.display()),
            )
            .into());
        }

        if let Some(expected_count) = frame_count {
            if paths.len() != expected_count {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "{} has {} frames; expected {expected_count}",
                        directory.display(),
                        paths.len()
                    ),
                )
                .into());
            }
        } else {
            frame_count = Some(paths.len());
        }

        let mut frames = Vec::with_capacity(paths.len());
        for path in paths {
            let frame = image::open(&path)?.to_rgba8();
            let dimensions = frame.dimensions();
            if let Some(expected_size) = frame_size {
                if dimensions != expected_size {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!(
                            "{} is {}x{}; expected {}x{}",
                            path.display(),
                            dimensions.0,
                            dimensions.1,
                            expected_size.0,
                            expected_size.1
                        ),
                    )
                    .into());
                }
            } else {
                frame_size = Some(dimensions);
            }
            frames.push(frame);
        }
        rows.push(frames);
    }

    let (frame_width, frame_height) = frame_size.expect("four non-empty direction folders");
    let frame_count = frame_count.expect("four non-empty direction folders");
    let columns = u32::try_from(frame_count)
        .map_err(|_| std::io::Error::other("too many spritesheet frames"))?;
    let sheet_width = frame_width
        .checked_mul(columns)
        .ok_or_else(|| std::io::Error::other("spritesheet width overflow"))?;
    let sheet_height = frame_height
        .checked_mul(rows.len() as u32)
        .ok_or_else(|| std::io::Error::other("spritesheet height overflow"))?;
    let mut image = RgbaImage::new(sheet_width, sheet_height);

    for (row, frames) in rows.iter().enumerate() {
        for (column, frame) in frames.iter().enumerate() {
            image.copy_from(
                frame,
                column as u32 * frame_width,
                row as u32 * frame_height,
            )?;
        }
    }

    Ok(PackedSheet {
        image,
        frame_width,
        frame_height,
        frame_count,
    })
}

fn natural_path_cmp(left: &Path, right: &Path) -> Ordering {
    let left_name = left
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    let right_name = right
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    let left = left_name.as_bytes();
    let right = right_name.as_bytes();
    let mut left_index = 0;
    let mut right_index = 0;

    while left_index < left.len() && right_index < right.len() {
        if left[left_index].is_ascii_digit() && right[right_index].is_ascii_digit() {
            let left_start = left_index;
            let right_start = right_index;
            while left_index < left.len() && left[left_index].is_ascii_digit() {
                left_index += 1;
            }
            while right_index < right.len() && right[right_index].is_ascii_digit() {
                right_index += 1;
            }

            let left_digits = &left[left_start..left_index];
            let right_digits = &right[right_start..right_index];
            let left_significant = left_digits
                .iter()
                .position(|digit| *digit != b'0')
                .map(|start| &left_digits[start..])
                .unwrap_or(&left_digits[left_digits.len() - 1..]);
            let right_significant = right_digits
                .iter()
                .position(|digit| *digit != b'0')
                .map(|start| &right_digits[start..])
                .unwrap_or(&right_digits[right_digits.len() - 1..]);
            let numeric_order = left_significant
                .len()
                .cmp(&right_significant.len())
                .then_with(|| left_significant.cmp(right_significant));
            if numeric_order != Ordering::Equal {
                return numeric_order;
            }
            let zero_padding_order = left_digits.len().cmp(&right_digits.len());
            if zero_padding_order != Ordering::Equal {
                return zero_padding_order;
            }
        } else {
            let order = left[left_index].cmp(&right[right_index]);
            if order != Ordering::Equal {
                return order;
            }
            left_index += 1;
            right_index += 1;
        }
    }

    (left.len() - left_index).cmp(&(right.len() - right_index))
}

fn resize_to_target(image: RgbaImage, width: u32, height: u32) -> RgbaImage {
    if image.dimensions() == (width, height) {
        image
    } else {
        image::imageops::resize(&image, width, height, FilterType::Triangle)
    }
}

fn parse_dimensions(stem: &str) -> Option<(u32, u32)> {
    stem.split('_').find_map(|part| {
        let (width, height) = part.split_once('x')?;
        let width = width.parse::<u32>().ok()?;
        let height = height.parse::<u32>().ok()?;
        (width > 0 && height > 0).then_some((width, height))
    })
}

fn strip_white_background(image: &mut RgbaImage, threshold: u8) {
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return;
    }

    let mut visited = vec![false; (width * height) as usize];
    let mut queue = VecDeque::new();
    let is_near_white =
        |pixel: &Rgba<u8>| pixel[0] >= threshold && pixel[1] >= threshold && pixel[2] >= threshold;

    for x in 0..width {
        queue.push_back((x, 0));
        queue.push_back((x, height - 1));
    }
    for y in 0..height {
        queue.push_back((0, y));
        queue.push_back((width - 1, y));
    }

    while let Some((x, y)) = queue.pop_front() {
        let index = (y * width + x) as usize;
        if visited[index] {
            continue;
        }
        visited[index] = true;

        if is_near_white(image.get_pixel(x, y)) {
            image.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            if x > 0 {
                queue.push_back((x - 1, y));
            }
            if x + 1 < width {
                queue.push_back((x + 1, y));
            }
            if y > 0 {
                queue.push_back((x, y - 1));
            }
            if y + 1 < height {
                queue.push_back((x, y + 1));
            }
        }
    }

    let snapshot = image.clone();
    for y in 0..height {
        for x in 0..width {
            let pixel = snapshot.get_pixel(x, y);
            if pixel[3] == 0 {
                continue;
            }
            let touches_alpha = (x > 0 && snapshot.get_pixel(x - 1, y)[3] == 0)
                || (x + 1 < width && snapshot.get_pixel(x + 1, y)[3] == 0)
                || (y > 0 && snapshot.get_pixel(x, y - 1)[3] == 0)
                || (y + 1 < height && snapshot.get_pixel(x, y + 1)[3] == 0);

            let max_channel = pixel[0].max(pixel[1]).max(pixel[2]);
            let min_channel = pixel[0].min(pixel[1]).min(pixel[2]);
            if touches_alpha && max_channel > 165 && max_channel - min_channel < 40 {
                *image.get_pixel_mut(x, y) = Rgba([0, 0, 0, 0]);
            }
        }
    }
}

fn snap_colors(image: &mut RgbaImage, target: [u8; 3], tolerance: i16) {
    for pixel in image.pixels_mut() {
        let red_delta = (pixel[0] as i16 - target[0] as i16).abs();
        let green_delta = (pixel[1] as i16 - target[1] as i16).abs();
        let blue_delta = (pixel[2] as i16 - target[2] as i16).abs();
        if red_delta <= tolerance && green_delta <= tolerance && blue_delta <= tolerance {
            *pixel = Rgba([target[0], target[1], target[2], 255]);
        }
    }
}

fn process_tile(image: &mut RgbaImage, snap_color: Option<[u8; 3]>) {
    if let Some(target) = snap_color {
        snap_colors(image, target, 26);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        natural_path_cmp, pack_animation_sheet, parse_dimensions, parse_rgb, process_tile,
        resize_to_target, snap_colors, strip_white_background,
    };
    use image::{Rgba, RgbaImage};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_directory() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after UNIX epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "clachan-forge-pack-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("temporary test directory can be created");
        path
    }

    fn write_frame(directory: &Path, frame_number: u8, direction: u8) {
        let mut image = RgbaImage::from_pixel(2, 1, Rgba([0, 0, 0, 0]));
        image.put_pixel(
            0,
            0,
            Rgba([direction * 20 + frame_number, direction, frame_number, 255]),
        );
        image.put_pixel(1, 0, Rgba([11, 22, 33, 0]));
        image
            .save(directory.join(format!("frame_{frame_number:02}.png")))
            .expect("test frame can be saved");
    }

    #[test]
    fn parses_dimensions_and_rejects_zero_sized_images() {
        assert_eq!(parse_dimensions("dirt_road_48x32_tile"), Some((48, 32)));
        assert_eq!(parse_dimensions("bad_0x32_sprite"), None);
        assert_eq!(parse_dimensions("no_dimensions"), None);
    }

    #[test]
    fn natural_filename_order_compares_numeric_chunks() {
        assert!(natural_path_cmp(Path::new("frame_2.png"), Path::new("frame_10.png")).is_lt());
    }

    #[test]
    fn packs_directions_in_engine_order_without_changing_rgba_pixels() {
        let root = temporary_directory();
        for (direction_index, direction) in ["south", "north", "west", "east"].iter().enumerate() {
            let directory = root.join(direction);
            fs::create_dir_all(&directory).unwrap();
            write_frame(&directory, 10, direction_index as u8);
            write_frame(&directory, 2, direction_index as u8);
        }

        let packed = pack_animation_sheet(&root).expect("four-row sheet packs");
        assert_eq!(packed.image.dimensions(), (4, 4));
        assert_eq!((packed.frame_width, packed.frame_height), (2, 1));
        assert_eq!(packed.frame_count, 2);
        for row in 0..4 {
            let direction = row as u8;
            assert_eq!(
                packed.image.get_pixel(0, row),
                &Rgba([direction * 20 + 2, direction, 2, 255])
            );
            assert_eq!(
                packed.image.get_pixel(2, row),
                &Rgba([direction * 20 + 10, direction, 10, 255])
            );
            assert_eq!(packed.image.get_pixel(1, row), &Rgba([11, 22, 33, 0]));
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn skips_resampling_for_native_dimensions() {
        let image = RgbaImage::from_fn(32, 32, |x, y| Rgba([x as u8, y as u8, (x ^ y) as u8, 255]));
        let original = image.clone();

        assert_eq!(resize_to_target(image, 32, 32), original);
    }

    #[test]
    fn strips_edge_connected_white_but_preserves_interior_white() {
        let mut image = RgbaImage::from_pixel(3, 3, Rgba([255, 255, 255, 255]));
        image.put_pixel(1, 1, Rgba([255, 0, 0, 255]));

        strip_white_background(&mut image, 238);

        assert_eq!(image.get_pixel(0, 0), &Rgba([0, 0, 0, 0]));
        assert_eq!(image.get_pixel(1, 1), &Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn removes_one_pixel_bright_neutral_fringe_next_to_transparency() {
        let mut image = RgbaImage::from_pixel(3, 1, Rgba([0, 0, 0, 255]));
        image.put_pixel(0, 0, Rgba([255, 255, 255, 255]));
        image.put_pixel(1, 0, Rgba([190, 190, 190, 255]));
        image.put_pixel(2, 0, Rgba([150, 130, 110, 255]));

        strip_white_background(&mut image, 238);

        assert_eq!(image.get_pixel(0, 0), &Rgba([0, 0, 0, 0]));
        assert_eq!(image.get_pixel(1, 0), &Rgba([0, 0, 0, 0]));
        assert_eq!(image.get_pixel(2, 0), &Rgba([150, 130, 110, 255]));
    }

    #[test]
    fn preserves_full_rgb_on_tiles_by_default() {
        let mut image = RgbaImage::new(3, 1);
        image.put_pixel(0, 0, Rgba([88, 142, 68, 255]));
        image.put_pixel(1, 0, Rgba([145, 72, 150, 255]));
        image.put_pixel(2, 0, Rgba([206, 214, 224, 255]));
        let original = image.clone();

        process_tile(&mut image, None);

        assert_eq!(image, original);
        assert_eq!(image.get_pixel(0, 0), &Rgba([88, 142, 68, 255]));
        assert_eq!(image.get_pixel(1, 0), &Rgba([145, 72, 150, 255]));
        assert_eq!(image.get_pixel(2, 0), &Rgba([206, 214, 224, 255]));
    }

    #[test]
    fn snap_color_is_available_only_when_explicitly_requested() {
        let mut image = RgbaImage::from_pixel(2, 1, Rgba([80, 140, 70, 255]));

        snap_colors(&mut image, [88, 142, 68], 26);

        assert_eq!(image.get_pixel(0, 0), &Rgba([88, 142, 68, 255]));
        assert_eq!(image.get_pixel(1, 0), &Rgba([88, 142, 68, 255]));
        assert_eq!(parse_rgb("88,142,68").unwrap(), [88, 142, 68]);
    }
}
