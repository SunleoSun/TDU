use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use tdu_assets::procedural::{
    TextureRecipe, WorldChunk, chunk_cache_key, generate_asphalt_chunk, generate_asphalt_region,
};
use tdu_formats::texture_2db::Texture2Db;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os();
    let _exe = args.next();

    let Some(command) = args.next() else {
        print_usage();
        return Ok(());
    };

    match command.to_string_lossy().as_ref() {
        "texture-info" => {
            let input = required_path(args.next(), "input .2DB path")?;
            ensure_no_extra_args(args)?;
            texture_info(&input)
        }
        "texture-export" => {
            let input = required_path(args.next(), "input .2DB path")?;
            let output = required_path(args.next(), "output .png path")?;
            ensure_no_extra_args(args)?;
            texture_export(&input, &output)
        }
        "procedural-asphalt" => {
            let seed = parse_seed(args.next(), "world seed")?;
            let chunk_x = parse_i32(args.next(), "chunk x")?;
            let chunk_y = parse_i32(args.next(), "chunk y")?;
            let resolution = parse_resolution(args.next())?;
            let output = required_path(args.next(), "output .png path")?;
            ensure_no_extra_args(args)?;
            procedural_asphalt(seed, chunk_x, chunk_y, resolution, &output)
        }
        "procedural-region" => {
            let seed = parse_seed(args.next(), "world seed")?;
            let chunk_x = parse_i32(args.next(), "center chunk x")?;
            let chunk_y = parse_i32(args.next(), "center chunk y")?;
            let resolution = parse_resolution(args.next())?;
            let radius = required_text(args.next(), "radius")?
                .parse::<u32>()
                .map_err(|error| format!("invalid radius: {error}"))?;
            let output = required_path(args.next(), "output .png path")?;
            ensure_no_extra_args(args)?;
            procedural_region(seed, chunk_x, chunk_y, resolution, radius, &output)
        }
        "-h" | "--help" | "help" => {
            print_usage();
            Ok(())
        }
        other => Err(format!("unknown command '{other}'").into()),
    }
}

fn required_path(value: Option<OsString>, label: &str) -> Result<PathBuf, Box<dyn Error>> {
    value
        .map(PathBuf::from)
        .ok_or_else(|| format!("missing {label}").into())
}

fn required_text(value: Option<OsString>, label: &str) -> Result<String, Box<dyn Error>> {
    value
        .map(|value| value.to_string_lossy().into_owned())
        .ok_or_else(|| format!("missing {label}").into())
}

fn parse_seed(value: Option<OsString>, label: &str) -> Result<u64, Box<dyn Error>> {
    let value = required_text(value, label)?;
    let parsed = if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16)
    } else {
        value.parse()
    };
    parsed.map_err(|error| format!("invalid {label} '{value}': {error}").into())
}

fn parse_i32(value: Option<OsString>, label: &str) -> Result<i32, Box<dyn Error>> {
    let value = required_text(value, label)?;
    value
        .parse()
        .map_err(|error| format!("invalid {label} '{value}': {error}").into())
}

fn parse_resolution(value: Option<OsString>) -> Result<u32, Box<dyn Error>> {
    let value = required_text(value, "resolution")?;
    let resolution: u32 = value
        .parse()
        .map_err(|error| format!("invalid resolution '{value}': {error}"))?;
    if !(64..=4096).contains(&resolution) || !resolution.is_power_of_two() {
        return Err("resolution must be a power of two from 64 through 4096".into());
    }
    Ok(resolution)
}

fn ensure_no_extra_args(mut args: impl Iterator<Item = OsString>) -> Result<(), Box<dyn Error>> {
    if let Some(extra) = args.next() {
        return Err(format!("unexpected argument '{}'", extra.to_string_lossy()).into());
    }
    Ok(())
}

fn texture_info(path: &Path) -> Result<(), Box<dyn Error>> {
    let bytes = fs::read(path)?;
    let texture = Texture2Db::parse(&bytes)?;

    println!("file: {}", path.display());
    println!("name: {}", texture.name);
    println!("size: {}x{}", texture.width, texture.height);
    println!("mips: {}", texture.mip_count);
    println!("header mip shadow: {}", texture.header_mip_shadow);
    println!("compression: {}", texture.compression);
    println!("declared file size: {}", texture.declared_file_size);
    println!("actual file size: {}", bytes.len());
    println!("compressed payload: {}", texture.payload().len());
    println!("base mip bytes: {}", texture.base_mip_len());
    Ok(())
}

fn texture_export(input: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
    let bytes = fs::read(input)?;
    let texture = Texture2Db::parse(&bytes)?;
    let rgba = texture.decode_base_rgba8()?;
    write_png(output, texture.width as u32, texture.height as u32, &rgba)?;

    println!(
        "exported {} ({}x{}, {}) -> {}",
        texture.name,
        texture.width,
        texture.height,
        texture.compression,
        output.display()
    );
    Ok(())
}

fn procedural_asphalt(
    seed: u64,
    chunk_x: i32,
    chunk_y: i32,
    resolution: u32,
    output: &Path,
) -> Result<(), Box<dyn Error>> {
    let recipe = TextureRecipe {
        world_seed: seed,
        texels_per_chunk: resolution,
        ..TextureRecipe::default()
    };
    let chunk = WorldChunk {
        x: chunk_x,
        y: chunk_y,
    };
    let generated = generate_asphalt_chunk(&recipe, chunk);
    write_png(output, generated.width, generated.height, &generated.rgba)?;

    println!(
        "generated asphalt seed=0x{seed:016x} chunk=({chunk_x},{chunk_y}) {}x{} cache_key=0x{:016x} -> {}",
        generated.width,
        generated.height,
        chunk_cache_key(&recipe, chunk),
        output.display()
    );
    Ok(())
}

fn procedural_region(
    seed: u64,
    chunk_x: i32,
    chunk_y: i32,
    resolution: u32,
    radius: u32,
    output: &Path,
) -> Result<(), Box<dyn Error>> {
    if radius > 4 {
        return Err("radius must be between 0 and 4".into());
    }

    let recipe = TextureRecipe {
        world_seed: seed,
        texels_per_chunk: resolution,
        ..TextureRecipe::default()
    };
    let center = WorldChunk {
        x: chunk_x,
        y: chunk_y,
    };
    let started = Instant::now();
    let generated = generate_asphalt_region(&recipe, center, radius);
    let generation_ms = started.elapsed().as_secs_f64() * 1000.0;
    write_png(output, generated.width, generated.height, &generated.rgba)?;

    println!(
        "generated asphalt region seed=0x{seed:016x} center=({chunk_x},{chunk_y}) radius={radius} {}x{} in {generation_ms:.1} ms -> {}",
        generated.width,
        generated.height,
        output.display()
    );
    Ok(())
}

fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }

    let file = fs::File::create(path)?;
    let writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(rgba)?;
    Ok(())
}

fn print_usage() {
    println!(
        "SunTDU CLI\n\n\
         Usage:\n  \
         tdu-cli texture-info <input.2DB>\n  \
         tdu-cli texture-export <input.2DB> <output.png>\n  \
                  tdu-cli procedural-asphalt <seed|0xhex> <chunk-x> <chunk-y> <resolution> <output.png>\n  \
         tdu-cli procedural-region <seed|0xhex> <center-x> <center-y> <resolution> <radius> <output.png>"
    );
}
