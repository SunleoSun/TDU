use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::Instant;

use eframe::egui;
use tdu_assets::procedural::{
    GeneratedTexture, TextureRecipe, WorldChunk, chunk_cache_key, chunk_seed,
    generate_asphalt_region,
};
use tdu_formats::texture_2db::{BlockCompression, Texture2Db};

const DEFAULT_TEXTURE: &str = "game/Euro/Bnk/FX/setgrass.2DB";
const PREVIEW_RADIUS: u32 = 1;

fn main() -> eframe::Result<()> {
    let initial_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_TEXTURE.to_owned());

    eframe::run_native(
        "SunTDU Asset Lab",
        eframe::NativeOptions::default(),
        Box::new(move |cc| Ok(Box::new(TextureViewerApp::new(cc, initial_path.clone())))),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Procedural,
    TduTexture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextureChannel {
    Color,
    Alpha,
}

struct TextureMetadata {
    name: String,
    width: u16,
    height: u16,
    mip_count: u8,
    compression: BlockCompression,
}

struct LoadedTexture {
    rgba: Vec<u8>,
    width: u16,
    height: u16,
}

struct ProceduralPreview {
    recipe: TextureRecipe,
    center: WorldChunk,
    generated: Option<GeneratedTexture>,
    texture: Option<egui::TextureHandle>,
    zoom: f32,
    show_boundaries: bool,
    dirty: bool,
    status: String,
}

impl ProceduralPreview {
    fn new(ctx: &egui::Context) -> Self {
        let recipe = TextureRecipe {
            texels_per_chunk: 256,
            ..TextureRecipe::default()
        };

        let mut state = Self {
            recipe,
            center: WorldChunk { x: 0, y: 0 },
            generated: None,
            texture: None,
            zoom: 1.0,
            show_boundaries: true,
            dirty: false,
            status: String::new(),
        };
        state.regenerate(ctx);
        state
    }

    fn regenerate(&mut self, ctx: &egui::Context) {
        let started = Instant::now();
        let generated = generate_asphalt_region(&self.recipe, self.center, PREVIEW_RADIUS);
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [generated.width as usize, generated.height as usize],
            &generated.rgba,
        );
        self.texture = Some(ctx.load_texture(
            "procedural-asphalt-preview",
            image,
            egui::TextureOptions::LINEAR,
        ));
        self.dirty = false;
        self.status = format!(
            "{}x{} generated in {:.1} ms",
            generated.width,
            generated.height,
            started.elapsed().as_secs_f64() * 1000.0
        );
        self.generated = Some(generated);
    }

    fn save_png(&mut self) -> Result<PathBuf, String> {
        if self.dirty {
            return Err("settings changed; regenerate before saving".to_owned());
        }
        let Some(generated) = &self.generated else {
            return Err("nothing generated yet".to_owned());
        };

        let output_dir = Path::new("assets").join("generated").join("previews");
        fs::create_dir_all(&output_dir).map_err(|error| error.to_string())?;
        let output = output_dir.join(format!(
            "asphalt-v{}-seed{:016x}-x{}-y{}-{}px.png",
            self.recipe.version,
            self.recipe.world_seed,
            self.center.x,
            self.center.y,
            self.recipe.texels_per_chunk
        ));

        let file = fs::File::create(&output).map_err(|error| error.to_string())?;
        let writer = BufWriter::new(file);
        let mut encoder = png::Encoder::new(writer, generated.width, generated.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .and_then(|mut png| png.write_image_data(&generated.rgba))
            .map_err(|error| error.to_string())?;

        Ok(output)
    }
}

struct TextureViewerApp {
    mode: ViewMode,
    path: String,
    loaded_path: Option<String>,
    metadata: Option<TextureMetadata>,
    loaded: Option<LoadedTexture>,
    texture: Option<egui::TextureHandle>,
    channel: TextureChannel,
    zoom: f32,
    checkerboard: bool,
    error: Option<String>,
    procedural: ProceduralPreview,
}

impl TextureViewerApp {
    fn new(cc: &eframe::CreationContext<'_>, initial_path: String) -> Self {
        let mut app = Self {
            mode: ViewMode::Procedural,
            path: initial_path,
            loaded_path: None,
            metadata: None,
            loaded: None,
            texture: None,
            channel: TextureChannel::Color,
            zoom: 2.0,
            checkerboard: true,
            error: None,
            procedural: ProceduralPreview::new(&cc.egui_ctx),
        };

        if Path::new(&app.path).is_file() {
            app.load_tdu_texture(&cc.egui_ctx);
        }
        app
    }

    fn load_tdu_texture(&mut self, ctx: &egui::Context) {
        self.error = None;
        self.texture = None;
        self.metadata = None;
        self.loaded = None;
        self.loaded_path = None;

        let result = (|| -> Result<(), String> {
            let bytes = fs::read(&self.path).map_err(|error| error.to_string())?;
            let texture = Texture2Db::parse(&bytes).map_err(|error| error.to_string())?;
            let rgba = texture
                .decode_base_rgba8()
                .map_err(|error| error.to_string())?;

            self.metadata = Some(TextureMetadata {
                name: texture.name.clone(),
                width: texture.width,
                height: texture.height,
                mip_count: texture.mip_count,
                compression: texture.compression,
            });
            self.loaded = Some(LoadedTexture {
                rgba,
                width: texture.width,
                height: texture.height,
            });
            self.loaded_path = Some(self.path.clone());
            self.refresh_tdu_texture(ctx);
            Ok(())
        })();

        if let Err(error) = result {
            self.error = Some(error);
        }
    }

    fn refresh_tdu_texture(&mut self, ctx: &egui::Context) {
        let Some(loaded) = &self.loaded else {
            return;
        };

        let pixels = match self.channel {
            TextureChannel::Color => loaded.rgba.clone(),
            TextureChannel::Alpha => {
                let mut alpha = Vec::with_capacity(loaded.rgba.len());
                for pixel in loaded.rgba.chunks_exact(4) {
                    alpha.extend_from_slice(&[pixel[3], pixel[3], pixel[3], 255]);
                }
                alpha
            }
        };

        let image = egui::ColorImage::from_rgba_unmultiplied(
            [loaded.width as usize, loaded.height as usize],
            &pixels,
        );
        self.texture =
            Some(ctx.load_texture("tdu-texture-preview", image, egui::TextureOptions::NEAREST));
    }

    fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.mode, ViewMode::Procedural, "Procedural lab");
                ui.selectable_value(&mut self.mode, ViewMode::TduTexture, "TDU .2DB");
                ui.separator();
                ui.label("SunTDU Asset Lab");
            });
        });
    }

    fn procedural_ui(&mut self, ctx: &egui::Context) {
        let mut settings_changed = false;
        let mut regenerate_now = false;

        egui::SidePanel::left("procedural_controls")
            .resizable(true)
            .default_width(310.0)
            .show(ctx, |ui| {
                ui.heading("Asphalt recipe");
                ui.label("Continuous world-space fields + scattered stone aggregate + sparse branched cracks.");
                ui.separator();

                ui.label("World seed");
                settings_changed |= ui
                    .add(egui::DragValue::new(&mut self.procedural.recipe.world_seed).speed(1.0))
                    .changed();

                ui.horizontal(|ui| {
                    ui.label("Center chunk");
                    settings_changed |= ui
                        .add(egui::DragValue::new(&mut self.procedural.center.x))
                        .changed();
                    settings_changed |= ui
                        .add(egui::DragValue::new(&mut self.procedural.center.y))
                        .changed();
                });

                ui.horizontal(|ui| {
                    ui.label("Resolution");
                    egui::ComboBox::from_id_salt("proc_resolution")
                        .selected_text(format!(
                            "{} px/chunk",
                            self.procedural.recipe.texels_per_chunk
                        ))
                        .show_ui(ui, |ui| {
                            for size in [128, 256, 512, 1024] {
                                settings_changed |= ui
                                    .selectable_value(
                                        &mut self.procedural.recipe.texels_per_chunk,
                                        size,
                                        format!("{size}"),
                                    )
                                    .changed();
                            }
                        });
                });

                ui.separator();
                settings_changed |= ui
                    .add(
                        egui::Slider::new(
                            &mut self.procedural.recipe.aggregate_strength,
                            0.70..=1.0,
                        )
                        .text("Aggregate grains"),
    )
    .changed();
settings_changed |= ui
    .add(
        egui::Slider::new(&mut self.procedural.recipe.asphalt_tone, 0.0..=1.0)
            .text("Road tone (light to dark)"),
                    )
                    .changed();
                settings_changed |= ui
                    .add(
                        egui::Slider::new(&mut self.procedural.recipe.macro_strength, 0.0..=0.30)
                            .text("Macro variation"),
                    )
                    .changed();
                settings_changed |= ui
                    .add(
                        egui::Slider::new(&mut self.procedural.recipe.dirt_strength, 0.0..=0.40)
                            .text("Dirt"),
                    )
                    .changed();
                settings_changed |= ui
                    .add(
                        egui::Slider::new(&mut self.procedural.recipe.crack_strength, 0.25..=0.50)
                            .text("Crack darkness"),
                    )
                    .changed();
settings_changed |= ui
    .add(
        egui::Slider::new(&mut self.procedural.recipe.crack_coverage, 0.0..=1.0)
            .text("Crack zones"),
    )
    .changed();
settings_changed |= ui
    .add(
        egui::Slider::new(&mut self.procedural.recipe.crack_density, 0.0..=1.0)
            .text("Crack density"),
    )
    .changed();
ui.add_space(0.0);

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Regenerate").clicked() {
                        regenerate_now = true;
                    }
                    if ui.button("Next seed").clicked() {
                        self.procedural.recipe.world_seed =
                            self.procedural.recipe.world_seed.wrapping_add(1);
                        regenerate_now = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Zoom");
                    ui.add(egui::Slider::new(&mut self.procedural.zoom, 0.1..=4.0));
                });
                ui.checkbox(
                    &mut self.procedural.show_boundaries,
                    "Show chunk boundaries",
                );

                if ui.button("Save preview PNG").clicked() {
                    self.procedural.status = match self.procedural.save_png() {
                        Ok(path) => format!("saved {}", path.display()),
                        Err(error) => format!("save failed: {error}"),
                    };
                }

                ui.separator();
                ui.label(&self.procedural.status);

                let center_seed = chunk_seed(&self.procedural.recipe, self.procedural.center);
                let cache_key = chunk_cache_key(&self.procedural.recipe, self.procedural.center);
                ui.monospace(format!("center seed: {center_seed:016x}"));
                ui.monospace(format!("cache key:  {cache_key:016x}"));

                ui.collapsing("3x3 chunk seeds", |ui| {
                    egui::Grid::new("chunk_seed_grid").show(ui, |ui| {
                        for dy in -1..=1 {
                            for dx in -1..=1 {
                                let chunk = WorldChunk {
                                    x: self.procedural.center.x + dx,
                                    y: self.procedural.center.y + dy,
                                };
                                ui.monospace(format!(
                                    "({:+},{:+}) {:08x}",
                                    chunk.x,
                                    chunk.y,
                                    chunk_seed(&self.procedural.recipe, chunk) as u32
                                ));
                            }
                            ui.end_row();
                        }
                    });
                });
            });

        if settings_changed {
            self.procedural.dirty = true;
            self.procedural.status = "settings changed; press Regenerate".to_owned();
        }
        if regenerate_now {
            self.procedural.regenerate(ctx);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label(
                "The image is one continuous 3x3 region. Grid lines only mark chunk boundaries.",
            );
            ui.separator();

            if let (Some(texture), Some(generated)) =
                (&self.procedural.texture, &self.procedural.generated)
            {
                show_image(
                    ui,
                    texture,
                    generated.width,
                    generated.height,
                    self.procedural.zoom,
                    false,
                    self.procedural
                        .show_boundaries
                        .then_some(self.procedural.recipe.texels_per_chunk),
                );
            }
        });
    }

    fn tdu_texture_ui(&mut self, ctx: &egui::Context) {
        let mut load_requested = false;
        let mut channel_changed = false;

        egui::TopBottomPanel::top("texture_toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("2DB file:");
                let response = ui.text_edit_singleline(&mut self.path);
                if ui.button("Load").clicked()
                    || (response.lost_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter)))
                {
                    load_requested = true;
                }
            });

            ui.horizontal(|ui| {
                channel_changed |= ui
                    .selectable_value(&mut self.channel, TextureChannel::Color, "RGBA")
                    .changed();
                channel_changed |= ui
                    .selectable_value(&mut self.channel, TextureChannel::Alpha, "Alpha")
                    .changed();
                ui.checkbox(&mut self.checkerboard, "Checkerboard");
                ui.separator();
                if ui.button("100%").clicked() {
                    self.zoom = 1.0;
                }
                if ui.button("200%").clicked() {
                    self.zoom = 2.0;
                }
                if ui.button("400%").clicked() {
                    self.zoom = 4.0;
                }
                ui.add(egui::Slider::new(&mut self.zoom, 0.25..=8.0).text("Zoom"));
            });
        });

        if load_requested {
            self.load_tdu_texture(ctx);
        } else if channel_changed {
            self.refresh_tdu_texture(ctx);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(error) = &self.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
                ui.separator();
            }

            if let Some(metadata) = &self.metadata {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(&metadata.name);
                    ui.label(format!(
                        "{}x{} · {} mips · {}",
                        metadata.width, metadata.height, metadata.mip_count, metadata.compression
                    ));
                    if let Some(path) = &self.loaded_path {
                        ui.label(path);
                    }
                });
                ui.separator();
            }

            if let (Some(texture), Some(loaded)) = (&self.texture, &self.loaded) {
                show_image(
                    ui,
                    texture,
                    u32::from(loaded.width),
                    u32::from(loaded.height),
                    self.zoom,
                    self.checkerboard,
                    None,
                );
            } else if self.error.is_none() {
                ui.label("Enter a TDU .2DB path and press Load.");
            }
        });
    }
}

impl eframe::App for TextureViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.top_bar(ctx);
        match self.mode {
            ViewMode::Procedural => self.procedural_ui(ctx),
            ViewMode::TduTexture => self.tdu_texture_ui(ctx),
        }
    }
}

fn show_image(
    ui: &mut egui::Ui,
    texture: &egui::TextureHandle,
    width: u32,
    height: u32,
    zoom: f32,
    checkerboard: bool,
    chunk_size: Option<u32>,
) {
    egui::ScrollArea::both().show(ui, |ui| {
        let size = egui::vec2(width as f32 * zoom, height as f32 * zoom);
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());

        if checkerboard {
            paint_checkerboard(ui, rect);
        }

        ui.put(rect, egui::Image::new(texture).fit_to_exact_size(size));

        if let Some(chunk_size) = chunk_size {
            let painter = ui.painter();
            let scaled = chunk_size as f32 * zoom;
            let stroke = egui::Stroke::new(1.0, egui::Color32::from_white_alpha(150));
            for index in 1..3 {
                let offset = scaled * index as f32;
                painter.line_segment(
                    [
                        egui::pos2(rect.left() + offset, rect.top()),
                        egui::pos2(rect.left() + offset, rect.bottom()),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(rect.left(), rect.top() + offset),
                        egui::pos2(rect.right(), rect.top() + offset),
                    ],
                    stroke,
                );
            }
        }
    });
}

fn paint_checkerboard(ui: &egui::Ui, rect: egui::Rect) {
    let visible = rect.intersect(ui.clip_rect());
    if visible.is_negative() {
        return;
    }

    let tile = 24.0;
    let start_x = ((visible.left() - rect.left()) / tile).floor() as i32;
    let end_x = ((visible.right() - rect.left()) / tile).ceil() as i32;
    let start_y = ((visible.top() - rect.top()) / tile).floor() as i32;
    let end_y = ((visible.bottom() - rect.top()) / tile).ceil() as i32;
    let painter = ui.painter();

    for y in start_y..=end_y {
        for x in start_x..=end_x {
            let color = if (x + y) & 1 == 0 {
                egui::Color32::from_gray(55)
            } else {
                egui::Color32::from_gray(85)
            };
            let min = egui::pos2(rect.left() + x as f32 * tile, rect.top() + y as f32 * tile);
            let tile_rect = egui::Rect::from_min_size(min, egui::vec2(tile, tile));
            painter.rect_filled(tile_rect.intersect(rect), 0.0, color);
        }
    }
}
