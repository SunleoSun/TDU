//! Deterministic procedural material generation.
//!
//! Neighbouring chunks sample the same continuous world-space fields. Hashed
//! lattice anchors provide stable local variation without hard chunk seams.
use rayon::prelude::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorldChunk {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectMaterialKey {
    pub object_id: u64,
    pub material_slot: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextureRecipe {
    pub version: u32,
    pub world_seed: u64,
    pub texels_per_chunk: u32,
    pub asphalt_tone: f32,
    pub macro_strength: f32,
    pub dirt_strength: f32,
    pub crack_strength: f32,
    pub crack_coverage: f32,
    pub crack_density: f32,
    pub aggregate_strength: f32,
}

impl Default for TextureRecipe {
    fn default() -> Self {
        Self {
            version: 7,
            world_seed: 0x5355_4E54_4455_0001,
            texels_per_chunk: 1024,
            asphalt_tone: 0.50,
            macro_strength: 0.18,
            dirt_strength: 0.18,
            crack_strength: 0.38,
            crack_coverage: 0.75,
            crack_density: 0.70,
            aggregate_strength: 0.92,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedTexture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl GeneratedTexture {
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let offset = ((y * self.width + x) * 4) as usize;
        [
            self.rgba[offset],
            self.rgba[offset + 1],
            self.rgba[offset + 2],
            self.rgba[offset + 3],
        ]
    }
}

pub fn stable_object_seed(world_seed: u64, key: ObjectMaterialKey, recipe_version: u32) -> u64 {
    let object = mix64(key.object_id ^ 0x9E37_79B9_7F4A_7C15);
    let slot = mix64(u64::from(key.material_slot) ^ 0xD1B5_4A32_D192_ED03);
    let version = mix64(u64::from(recipe_version) ^ 0x94D0_49BB_1331_11EB);
    mix64(world_seed ^ object.rotate_left(17) ^ slot.rotate_left(31) ^ version)
}

pub fn chunk_seed(recipe: &TextureRecipe, chunk: WorldChunk) -> u64 {
    hash_lattice(
        recipe.world_seed ^ u64::from(recipe.version),
        i64::from(chunk.x),
        i64::from(chunk.y),
    )
}

pub fn chunk_cache_key(recipe: &TextureRecipe, chunk: WorldChunk) -> u64 {
    let mut key = chunk_seed(recipe, chunk);
    key = mix64(key ^ u64::from(recipe.texels_per_chunk));
    key = mix64(key ^ u64::from(recipe.asphalt_tone.to_bits()).rotate_left(3));
    key = mix64(key ^ u64::from(recipe.macro_strength.to_bits()));
    key = mix64(key ^ u64::from(recipe.dirt_strength.to_bits()).rotate_left(7));
    key = mix64(key ^ u64::from(recipe.crack_strength.to_bits()).rotate_left(13));
    key = mix64(key ^ u64::from(recipe.crack_coverage.to_bits()).rotate_left(19));
    key = mix64(key ^ u64::from(recipe.crack_density.to_bits()).rotate_left(23));
    mix64(key ^ u64::from(recipe.aggregate_strength.to_bits()).rotate_left(29))
}

pub fn generate_asphalt_chunk(recipe: &TextureRecipe, chunk: WorldChunk) -> GeneratedTexture {
    generate_asphalt_region(recipe, chunk, 0)
}

pub fn generate_asphalt_region(
    recipe: &TextureRecipe,
    center: WorldChunk,
    radius: u32,
) -> GeneratedTexture {
    let chunks_per_axis = radius
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .expect("procedural preview radius overflow");
    let width = recipe
        .texels_per_chunk
        .checked_mul(chunks_per_axis)
        .expect("procedural preview width overflow");
    let height = width;

    let start_chunk_x = i64::from(center.x) - i64::from(radius);
    let start_chunk_y = i64::from(center.y) - i64::from(radius);
    let chunk_size = i64::from(recipe.texels_per_chunk);
    let start_x = start_chunk_x * chunk_size;
    let start_y = start_chunk_y * chunk_size;
    let row_bytes = width as usize * 4;

    let mut rgba = vec![0u8; row_bytes * height as usize];
    rgba.par_chunks_mut(row_bytes)
        .enumerate()
        .for_each(|(row_index, row)| {
            let world_y = start_y + row_index as i64;
            for x in 0..width {
                let world_x = start_x + i64::from(x);
                let pixel = sample_asphalt(recipe, world_x, world_y);
                let offset = x as usize * 4;
                row[offset..offset + 4].copy_from_slice(&pixel);
            }
        });

    GeneratedTexture {
        width,
        height,
        rgba,
    }
}

fn asphalt_tone_factor(asphalt_tone: f32) -> f32 {
    lerp(1.28, 0.72, asphalt_tone.clamp(0.0, 1.0))
}

fn dirt_base_luma(asphalt_tone: f32) -> f32 {
    lerp(96.0, 50.0, asphalt_tone.clamp(0.0, 1.0))
}

fn sample_asphalt(recipe: &TextureRecipe, world_x: i64, world_y: i64) -> [u8; 4] {
    let chunk_scale = recipe.texels_per_chunk.max(1) as f32;
    let x = world_x as f32 / chunk_scale;
    let y = world_y as f32 / chunk_scale;
    let seed =
        mix64(recipe.world_seed ^ u64::from(recipe.version).wrapping_mul(0x9E37_79B9_7F4A_7C15));

    let macro_noise = fbm(seed ^ 0xC6BC_2796_92B5_C323, x / 1.08, y / 1.08, 3);
    let dirt = fbm(seed ^ 0xDB4F_0B91_75AE_2165, x / 0.48, y / 0.48, 2);
    let micro = value_noise(seed ^ 0xBBE0_5633_03A4_619F, x / 0.015, y / 0.015);

    let damage_large = fbm(seed ^ 0x46E3_82A7_2B10_9047, x / 2.9, y / 2.9, 2);
    let damage_local = value_noise(seed ^ 0x6A09_E667_F3BC_C909, x / 0.96, y / 0.96);
    let damage = damage_large * 0.78 + damage_local * 0.22;
    let coverage = recipe.crack_coverage.clamp(0.0, 1.0);
    let crack_density = recipe.crack_density.clamp(0.0, 1.0);
    let damage_threshold = 0.86 - coverage * 0.58;
    let damage_mask = smoothstep(damage_threshold, damage_threshold + 0.12, damage);

    let crack_mask = if damage_mask > 0.002 && recipe.crack_strength > 0.0 && crack_density > 0.0 {
        let major_cracks = polyline_crack_network(
            seed ^ 0x94D0_49BB_1331_11EB,
            x,
            y,
            0.58,
            0.95 * crack_density,
            0.0018,
        );
        let severe_damage = smoothstep(0.65, 0.88, damage);
        let minor_cracks = if severe_damage > 0.002 {
            polyline_crack_network(
                seed ^ 0x243F_6A88_85A3_08D3,
                x + 0.041,
                y - 0.067,
                0.34,
                0.70 * crack_density,
                0.0012,
            ) * severe_damage
                * 0.50
        } else {
            0.0
        };
        (major_cracks + minor_cracks).clamp(0.0, 1.0)
            * damage_mask
            * recipe.crack_strength.clamp(0.0, 1.0)
    } else {
        0.0
    };

    let micro_term = (micro - 0.5) * 6.0;
    let mut rgb = [
        105.0 + micro_term,
        105.0 + micro_term * 0.92,
        102.0 + micro_term * 0.80,
    ];

    apply_aggregate(
        &mut rgb,
        seed ^ 0xB7E1_5162_8AED_2A6B,
        x,
        y,
        recipe.aggregate_strength,
    );

    let asphalt_tone = recipe.asphalt_tone.clamp(0.0, 1.0);
    let tone_factor = asphalt_tone_factor(asphalt_tone);
    for channel in &mut rgb {
        *channel *= tone_factor;
    }

    let macro_delta = (macro_noise - 0.5) * 56.0 * recipe.macro_strength.clamp(0.0, 1.0);
    rgb[0] += macro_delta;
    rgb[1] += macro_delta;
    rgb[2] += macro_delta * 0.96;

    let dirt_amount = smoothstep(0.42, 0.76, dirt) * recipe.dirt_strength.clamp(0.0, 1.0);
    if dirt_amount > 0.0 {
        let dirt_mix = dirt_amount * 0.72;
        let dirt_luma = dirt_base_luma(asphalt_tone);
        let dirt_color = [
            dirt_luma + 8.0 + macro_delta * 0.08,
            dirt_luma,
            dirt_luma - 14.0 - macro_delta * 0.03,
        ];
        for (channel, overlay) in rgb.iter_mut().zip(dirt_color) {
            *channel = lerp(*channel, overlay, dirt_mix);
        }
    }

    let crack_darkening = crack_mask * 74.0;
    rgb[0] -= crack_darkening;
    rgb[1] -= crack_darkening;
    rgb[2] -= crack_darkening * 0.92;

    [
        byte_channel(rgb[0]),
        byte_channel(rgb[1]),
        byte_channel(rgb[2]),
        255,
    ]
}

fn apply_aggregate(rgb: &mut [f32; 3], seed: u64, x: f32, y: f32, strength: f32) {
    let strength = strength.clamp(0.0, 1.0);
    if strength <= 0.0 {
        return;
    }

    let coarse = scattered_aggregate_sample(seed, x, y, 0.022, 0.72, strength);
    let fine = scattered_aggregate_sample(
        seed ^ 0xD6E8_FEB8_6659_FD93,
        x + 0.0037,
        y - 0.0051,
        0.0105,
        0.54,
        strength * 0.58,
    );

    let mut weight = coarse.weight;
    let mut color = coarse.color;
    if fine.weight > weight {
        weight = fine.weight;
        color = fine.color;
    } else if fine.weight > 0.0 {
        let fine_mix = fine.weight * (1.0 - weight) * 0.55;
        for (channel, fine_channel) in color.iter_mut().zip(fine.color) {
            *channel = lerp(*channel, fine_channel, fine_mix);
        }
        weight = (weight + fine_mix).clamp(0.0, 1.0);
    }

    if weight > 0.0 {
        for (channel, stone_channel) in rgb.iter_mut().zip(color) {
            *channel = lerp(*channel, stone_channel, weight);
        }
    }

    let shadow = coarse.edge_shadow.max(fine.edge_shadow * 0.70);
    rgb[0] -= shadow * 9.0;
    rgb[1] -= shadow * 9.0;
    rgb[2] -= shadow * 8.0;
}

#[derive(Debug, Clone, Copy)]
struct AggregateSample {
    weight: f32,
    color: [f32; 3],
    edge_shadow: f32,
}

fn scattered_aggregate_sample(
    seed: u64,
    x: f32,
    y: f32,
    cell_size: f32,
    density: f32,
    strength: f32,
) -> AggregateSample {
    let scaled_x = x / cell_size;
    let scaled_y = y / cell_size;
    let base_x = scaled_x.floor() as i64;
    let base_y = scaled_y.floor() as i64;

    let mut best_weight = 0.0f32;
    let mut best_color = [0.0; 3];
    let mut edge_shadow = 0.0f32;

    for offset_y in -1..=1 {
        for offset_x in -1..=1 {
            let cell_x = base_x + offset_x;
            let cell_y = base_y + offset_y;
            let cell_hash = hash_lattice(seed, cell_x, cell_y);

            for slot in 0..3u64 {
                let hash = mix64(
                    cell_hash
                        ^ (slot + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                        ^ seed.rotate_left((slot as u32 * 11 + 7) & 63),
                );
                if hash_part(hash, 1) > density {
                    continue;
                }

                let center_x = cell_x as f32 + 0.06 + hash_part(hash, 7) * 0.88;
                let center_y = cell_y as f32 + 0.06 + hash_part(hash, 19) * 0.88;
                let size_scale = 0.70 + hash_part(hash, 31) * 0.60;
                let aspect = 0.78 + hash_part(hash, 43) * 0.44;
                let shear = (hash_part(hash, 53) - 0.5) * 0.52;

                let dx = scaled_x - center_x;
                let dy = scaled_y - center_y;
                let skewed_x = dx + dy * shear;
                let radius_x = 0.245 * size_scale * aspect;
                let radius_y = 0.245 * size_scale / aspect;
                let distance = (skewed_x / radius_x).powi(2) + (dy / radius_y).powi(2);

                if distance > 1.22 {
                    continue;
                }

                let body = 1.0 - smoothstep(0.64, 1.0, distance);
                let ring =
                    smoothstep(0.68, 0.98, distance) * (1.0 - smoothstep(0.98, 1.20, distance));
                edge_shadow = edge_shadow.max(ring * strength);

                let weight = body * strength;
                if weight <= best_weight {
                    continue;
                }

                let tone = hash_part(hash, 13);
                let warmth = hash_part(hash, 37) - 0.5;
                let class = hash_part(hash, 59);
                let mut lightness = 101.0 + tone * 58.0;
                if class < 0.10 {
                    lightness -= 25.0;
                } else if class > 0.89 {
                    lightness += 24.0;
                }

                best_color = [
                    lightness + warmth * 22.0,
                    lightness + warmth * 8.0,
                    lightness - warmth * 16.0,
                ];
                best_weight = weight;
            }
        }
    }

    AggregateSample {
        weight: best_weight,
        color: best_color,
        edge_shadow,
    }
}

fn crack_segment_count(source_hash: u64) -> u64 {
    4 + ((hash_part(source_hash, 35) * 6.0) as u64).min(5)
}

fn polyline_crack_network(
    seed: u64,
    x: f32,
    y: f32,
    cell_size: f32,
    density: f32,
    base_width: f32,
) -> f32 {
    const DIAGONAL: f32 = std::f32::consts::FRAC_1_SQRT_2;
    const DIRECTIONS: [(f32, f32); 16] = [
        (1.0, 0.0),
        (0.9239, 0.3827),
        (DIAGONAL, DIAGONAL),
        (0.3827, 0.9239),
        (0.0, 1.0),
        (-0.3827, 0.9239),
        (-DIAGONAL, DIAGONAL),
        (-0.9239, 0.3827),
        (-1.0, 0.0),
        (-0.9239, -0.3827),
        (-DIAGONAL, -DIAGONAL),
        (-0.3827, -0.9239),
        (0.0, -1.0),
        (0.3827, -0.9239),
        (DIAGONAL, -DIAGONAL),
        (0.9239, -0.3827),
    ];

    let cell_x = (x / cell_size).floor() as i64;
    let cell_y = (y / cell_size).floor() as i64;
    let mut intensity = 0.0f32;

    for offset_y in -1..=1 {
        for offset_x in -1..=1 {
            let source_x = cell_x + offset_x;
            let source_y = cell_y + offset_y;
            let source_hash = hash_lattice(seed, source_x, source_y);
            if hash_part(source_hash, 3) > density {
                continue;
            }

            let mut point_x =
                (source_x as f32 + 0.10 + hash_part(source_hash, 11) * 0.80) * cell_size;
            let mut point_y =
                (source_y as f32 + 0.10 + hash_part(source_hash, 29) * 0.80) * cell_size;
            let mut direction_index = (hash_part(source_hash, 47) * 16.0) as i32 & 15;
            let segment_count = crack_segment_count(source_hash);

            for segment in 0..segment_count {
                let segment_hash =
                    mix64(source_hash ^ (segment + 1).wrapping_mul(0xD1B5_4A32_D192_ED03));
                let turn = (hash_part(segment_hash, 5) * 5.0) as i32 - 2;
                direction_index = (direction_index + turn).rem_euclid(16);
                let direction = DIRECTIONS[direction_index as usize];
                let length = cell_size * (0.095 + hash_part(segment_hash, 23) * 0.025);
                let next_x = point_x + direction.0 * length;
                let next_y = point_y + direction.1 * length;

                if hash_part(segment_hash, 41) > 0.08 {
                    let distance_sq =
                        distance_sq_to_segment(x, y, point_x, point_y, next_x, next_y);
                    let width = base_width * (0.72 + hash_part(segment_hash, 57) * 0.70);
                    let outer_width = width * 2.7;
                    let line =
                        1.0 - smoothstep(width * width, outer_width * outer_width, distance_sq);
                    intensity = intensity.max(line);

                    if segment == 2 && hash_part(source_hash, 55) < 0.34 {
                        let branch_hash = mix64(source_hash ^ 0xA24B_AED4_963E_E407);
                        let branch_turn = if hash_part(branch_hash, 9) < 0.5 {
                            -4
                        } else {
                            4
                        };
                        let mut branch_direction = (direction_index + branch_turn).rem_euclid(16);
                        let mut branch_x = next_x;
                        let mut branch_y = next_y;
                        let branch_segment_count =
                            1 + ((hash_part(branch_hash, 27) * 3.0) as u64).min(2);

                        for branch_segment in 0..branch_segment_count {
                            let hash = mix64(
                                branch_hash
                                    ^ (branch_segment + 1).wrapping_mul(0x94D0_49BB_1331_11EB),
                            );
                            let turn = (hash_part(hash, 17) * 3.0) as i32 - 1;
                            branch_direction = (branch_direction + turn).rem_euclid(16);
                            let direction = DIRECTIONS[branch_direction as usize];
                            let length = cell_size * (0.065 + hash_part(hash, 35) * 0.025);
                            let branch_next_x = branch_x + direction.0 * length;
                            let branch_next_y = branch_y + direction.1 * length;
                            let distance_sq = distance_sq_to_segment(
                                x,
                                y,
                                branch_x,
                                branch_y,
                                branch_next_x,
                                branch_next_y,
                            );
                            let width = base_width * (0.58 + hash_part(hash, 49) * 0.44);
                            let outer_width = width * 2.5;
                            let line = 1.0
                                - smoothstep(width * width, outer_width * outer_width, distance_sq);
                            intensity = intensity.max(line * 0.78);
                            branch_x = branch_next_x;
                            branch_y = branch_next_y;
                        }
                    }
                }

                point_x = next_x;
                point_y = next_y;
            }
        }
    }

    intensity
}

fn distance_sq_to_segment(
    point_x: f32,
    point_y: f32,
    start_x: f32,
    start_y: f32,
    end_x: f32,
    end_y: f32,
) -> f32 {
    let segment_x = end_x - start_x;
    let segment_y = end_y - start_y;
    let length_sq = segment_x * segment_x + segment_y * segment_y;
    if length_sq <= f32::EPSILON {
        return (point_x - start_x).powi(2) + (point_y - start_y).powi(2);
    }

    let projection =
        ((point_x - start_x) * segment_x + (point_y - start_y) * segment_y) / length_sq;
    let t = projection.clamp(0.0, 1.0);
    let closest_x = start_x + segment_x * t;
    let closest_y = start_y + segment_y * t;
    (point_x - closest_x).powi(2) + (point_y - closest_y).powi(2)
}

fn fbm(seed: u64, x: f32, y: f32, octaves: u32) -> f32 {
    let mut amplitude = 0.5;
    let mut frequency = 1.0;
    let mut total = 0.0;
    let mut normalization = 0.0;

    for octave in 0..octaves {
        total += value_noise(
            seed ^ mix64(u64::from(octave) + 0x6A09_E667_F3BC_C909),
            x * frequency,
            y * frequency,
        ) * amplitude;
        normalization += amplitude;
        amplitude *= 0.5;
        frequency *= 2.03;
    }

    if normalization == 0.0 {
        0.5
    } else {
        total / normalization
    }
}

fn value_noise(seed: u64, x: f32, y: f32) -> f32 {
    let x0 = x.floor() as i64;
    let y0 = y.floor() as i64;
    let tx = smooth_curve(x - x.floor());
    let ty = smooth_curve(y - y.floor());

    let v00 = hash_unit(seed, x0, y0);
    let v10 = hash_unit(seed, x0 + 1, y0);
    let v01 = hash_unit(seed, x0, y0 + 1);
    let v11 = hash_unit(seed, x0 + 1, y0 + 1);

    let top = lerp(v00, v10, tx);
    let bottom = lerp(v01, v11, tx);
    lerp(top, bottom, ty)
}

fn hash_unit(seed: u64, x: i64, y: i64) -> f32 {
    hash_part(hash_lattice(seed, x, y), 40)
}

fn hash_part(value: u64, rotate: u32) -> f32 {
    let mantissa = (value.rotate_left(rotate) >> 40) as u32;
    mantissa as f32 / 16_777_215.0
}

fn hash_lattice(seed: u64, x: i64, y: i64) -> u64 {
    let x_bits = x as u64;
    let y_bits = y as u64;
    mix64(
        seed ^ mix64(x_bits.wrapping_mul(0x9E37_79B9_7F4A_7C15))
            ^ mix64(y_bits.wrapping_mul(0xC2B2_AE3D_27D4_EB4F)).rotate_left(29),
    )
}

fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn smooth_curve(value: f32) -> f32 {
    value * value * (3.0 - 2.0 * value)
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    if (edge1 - edge0).abs() <= f32::EPSILON {
        return if value >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    smooth_curve(t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn byte_channel(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_recipe() -> TextureRecipe {
        TextureRecipe {
            texels_per_chunk: 32,
            ..TextureRecipe::default()
        }
    }

    #[test]
    fn generation_is_byte_deterministic() {
        let recipe = tiny_recipe();
        let chunk = WorldChunk { x: 17, y: -9 };
        let first = generate_asphalt_chunk(&recipe, chunk);
        let second = generate_asphalt_chunk(&recipe, chunk);
        assert_eq!(first, second);
    }

    #[test]
    fn separately_generated_chunks_match_region_slices() {
        let recipe = tiny_recipe();
        let center = WorldChunk { x: 4, y: 7 };
        let region = generate_asphalt_region(&recipe, center, 1);
        let size = recipe.texels_per_chunk;

        for dy in -1..=1 {
            for dx in -1..=1 {
                let chunk = WorldChunk {
                    x: center.x + dx,
                    y: center.y + dy,
                };
                let standalone = generate_asphalt_chunk(&recipe, chunk);
                let region_chunk_x = (dx + 1) as u32;
                let region_chunk_y = (dy + 1) as u32;

                for y in 0..size {
                    for x in 0..size {
                        assert_eq!(
                            standalone.pixel(x, y),
                            region.pixel(region_chunk_x * size + x, region_chunk_y * size + y)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn stable_object_seed_changes_with_semantic_identity() {
        let key = ObjectMaterialKey {
            object_id: 42,
            material_slot: 3,
        };
        let seed = stable_object_seed(123, key, 1);
        assert_eq!(seed, stable_object_seed(123, key, 1));
        assert_ne!(seed, stable_object_seed(123, key, 2));
        assert_ne!(
            seed,
            stable_object_seed(
                123,
                ObjectMaterialKey {
                    object_id: 43,
                    material_slot: 3,
                },
                1,
            )
        );
    }

    #[test]
    fn cache_key_changes_with_recipe_parameters() {
        let recipe = tiny_recipe();
        let chunk = WorldChunk { x: 1, y: 2 };
        let first = chunk_cache_key(&recipe, chunk);

        let changed_dirt = TextureRecipe {
            dirt_strength: recipe.dirt_strength + 0.1,
            ..recipe
        };
        assert_ne!(first, chunk_cache_key(&changed_dirt, chunk));

        let changed_density = TextureRecipe {
            crack_density: (recipe.crack_density + 0.1).min(1.0),
            ..recipe
        };
        assert_ne!(first, chunk_cache_key(&changed_density, chunk));

        let changed_tone = TextureRecipe {
            asphalt_tone: (recipe.asphalt_tone + 0.1).min(1.0),
            ..recipe
        };
        assert_ne!(first, chunk_cache_key(&changed_tone, chunk));
    }

    #[test]
    fn resolution_changes_sampling_density_not_world_pattern() {
        let low = TextureRecipe {
            texels_per_chunk: 32,
            ..TextureRecipe::default()
        };
        let high = TextureRecipe {
            texels_per_chunk: 64,
            ..low
        };
        let chunk = WorldChunk { x: -3, y: 5 };
        let low_image = generate_asphalt_chunk(&low, chunk);
        let high_image = generate_asphalt_chunk(&high, chunk);

        for y in [0, 7, 15, 31] {
            for x in [0, 5, 17, 31] {
                assert_eq!(low_image.pixel(x, y), high_image.pixel(x * 2, y * 2));
            }
        }
    }

    #[test]
    fn aggregate_and_crack_controls_change_output() {
        let recipe = tiny_recipe();
        let chunk = WorldChunk { x: 0, y: 0 };

        let no_aggregate = TextureRecipe {
            aggregate_strength: 0.0,
            ..recipe
        };
        assert_ne!(
            generate_asphalt_chunk(&recipe, chunk),
            generate_asphalt_chunk(&no_aggregate, chunk)
        );

        let cracked = TextureRecipe {
            crack_strength: 1.0,
            crack_coverage: 1.0,
            crack_density: 1.0,
            ..recipe
        };
        let no_density = TextureRecipe {
            crack_density: 0.0,
            ..cracked
        };
        let no_cracks = TextureRecipe {
            crack_strength: 0.0,
            ..cracked
        };
        assert_ne!(
            generate_asphalt_region(&cracked, chunk, 1),
            generate_asphalt_region(&no_density, chunk, 1)
        );
        assert_eq!(
            generate_asphalt_region(&no_density, chunk, 1),
            generate_asphalt_region(&no_cracks, chunk, 1)
        );
    }

    fn mean_rgb_difference(a: &GeneratedTexture, b: &GeneratedTexture) -> f32 {
        assert_eq!(a.width, b.width);
        assert_eq!(a.height, b.height);
        let mut total = 0u64;
        let mut channels = 0u64;
        for (left, right) in a.rgba.chunks_exact(4).zip(b.rgba.chunks_exact(4)) {
            for channel in 0..3 {
                total += u64::from(left[channel].abs_diff(right[channel]));
                channels += 1;
            }
        }
        total as f32 / channels as f32
    }

    #[test]
    fn biome_tone_helpers_follow_road_tone() {
        assert!(asphalt_tone_factor(0.0) > asphalt_tone_factor(1.0));
        assert!(dirt_base_luma(0.0) > dirt_base_luma(1.0));
    }

    #[test]
    fn crack_lengths_have_wide_deterministic_range() {
        assert_eq!(crack_segment_count(0), 4);
        assert_eq!(crack_segment_count(u64::MAX), 9);
    }

    #[test]
    fn material_variation_controls_have_visible_effect() {
        let base = TextureRecipe {
            texels_per_chunk: 64,
            macro_strength: 0.0,
            dirt_strength: 0.0,
            ..TextureRecipe::default()
        };
        let chunk = WorldChunk { x: 3, y: -2 };
        let baseline = generate_asphalt_region(&base, chunk, 1);

        let macro_image = generate_asphalt_region(
            &TextureRecipe {
                macro_strength: 0.30,
                ..base
            },
            chunk,
            1,
        );
        let dirt_image = generate_asphalt_region(
            &TextureRecipe {
                dirt_strength: 0.40,
                ..base
            },
            chunk,
            1,
        );

        assert!(mean_rgb_difference(&baseline, &macro_image) > 1.0);
        assert!(mean_rgb_difference(&baseline, &dirt_image) > 1.0);
    }

    #[test]
    fn recipe_version_changes_generated_world() {
        let recipe = tiny_recipe();
        let changed = TextureRecipe {
            version: recipe.version + 1,
            ..recipe
        };
        let chunk = WorldChunk { x: 2, y: 3 };

        assert_ne!(
            generate_asphalt_chunk(&recipe, chunk),
            generate_asphalt_chunk(&changed, chunk)
        );
    }
}
