use std::error::Error;
use std::fmt;

const HEADER_LEN: usize = 88;
const FOOTER_LEN: usize = 8;
const MAGIC_OFFSET: usize = 12;
const MAGIC: &[u8; 8] = b".2DBBMAP";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockCompression {
    Bc1,
    Bc3,
}

impl fmt::Display for BlockCompression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bc1 => f.write_str("BC1/DXT1"),
            Self::Bc3 => f.write_str("BC3/DXT5"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Texture2DbError {
    TooSmall {
        actual: usize,
    },
    InvalidMagic,
    InvalidDimensions {
        width: u16,
        height: u16,
    },
    InvalidMipCount(u8),
    UnsupportedPayload {
        width: u16,
        height: u16,
        mip_count: u8,
        payload_len: usize,
    },
    TruncatedBaseMip {
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for Texture2DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooSmall { actual } => write!(f, "2DB file is too small: {actual} bytes"),
            Self::InvalidMagic => f.write_str("not a supported TDU .2DB BMAP texture"),
            Self::InvalidDimensions { width, height } => {
                write!(f, "invalid texture dimensions {width}x{height}")
            }
            Self::InvalidMipCount(count) => write!(f, "invalid mip count {count}"),
            Self::UnsupportedPayload {
                width,
                height,
                mip_count,
                payload_len,
            } => write!(
                f,
                "unsupported 2DB payload: {width}x{height}, {mip_count} mips, {payload_len} bytes"
            ),
            Self::TruncatedBaseMip { expected, actual } => write!(
                f,
                "texture payload is truncated: need {expected} bytes for base mip, got {actual}"
            ),
        }
    }
}

impl Error for Texture2DbError {}

#[derive(Debug)]
pub struct Texture2Db<'a> {
    pub declared_file_size: u32,
    pub name: String,
    pub width: u16,
    pub height: u16,
    pub mip_count: u8,
    pub header_mip_shadow: u8,
    pub compression: BlockCompression,
    payload: &'a [u8],
}

impl<'a> Texture2Db<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Texture2DbError> {
        if bytes.len() < HEADER_LEN + FOOTER_LEN {
            return Err(Texture2DbError::TooSmall {
                actual: bytes.len(),
            });
        }

        if bytes.get(MAGIC_OFFSET..MAGIC_OFFSET + MAGIC.len()) != Some(MAGIC.as_slice()) {
            return Err(Texture2DbError::InvalidMagic);
        }

        let declared_file_size = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let name = String::from_utf8_lossy(&bytes[32..40])
            .trim_end_matches('\0')
            .to_owned();
        let width = u16::from_le_bytes(bytes[40..42].try_into().unwrap());
        let height = u16::from_le_bytes(bytes[42..44].try_into().unwrap());
        let mip_count = bytes[46];
        let header_mip_shadow = bytes[47];

        if width == 0 || height == 0 {
            return Err(Texture2DbError::InvalidDimensions { width, height });
        }
        if mip_count == 0 {
            return Err(Texture2DbError::InvalidMipCount(mip_count));
        }

        let payload = &bytes[HEADER_LEN..bytes.len() - FOOTER_LEN];
        let bc1_len = expected_block_payload_len(width, height, mip_count, 8);
        let bc3_len = expected_block_payload_len(width, height, mip_count, 16);

        let compression = if payload.len() == bc1_len {
            BlockCompression::Bc1
        } else if payload.len() == bc3_len {
            BlockCompression::Bc3
        } else {
            return Err(Texture2DbError::UnsupportedPayload {
                width,
                height,
                mip_count,
                payload_len: payload.len(),
            });
        };

        Ok(Self {
            declared_file_size,
            name,
            width,
            height,
            mip_count,
            header_mip_shadow,
            compression,
            payload,
        })
    }

    pub fn payload(&self) -> &'a [u8] {
        self.payload
    }

    pub fn base_mip_len(&self) -> usize {
        let block_bytes = match self.compression {
            BlockCompression::Bc1 => 8,
            BlockCompression::Bc3 => 16,
        };
        block_level_len(self.width, self.height, block_bytes)
    }

    pub fn decode_base_rgba8(&self) -> Result<Vec<u8>, Texture2DbError> {
        let expected = self.base_mip_len();
        if self.payload.len() < expected {
            return Err(Texture2DbError::TruncatedBaseMip {
                expected,
                actual: self.payload.len(),
            });
        }

        let mut rgba = vec![0; self.width as usize * self.height as usize * 4];
        let block_bytes = match self.compression {
            BlockCompression::Bc1 => 8,
            BlockCompression::Bc3 => 16,
        };
        let blocks_x = (self.width as usize).div_ceil(4);
        let blocks_y = (self.height as usize).div_ceil(4);

        for by in 0..blocks_y {
            for bx in 0..blocks_x {
                let block_index = by * blocks_x + bx;
                let start = block_index * block_bytes;
                let block = &self.payload[start..start + block_bytes];

                let pixels = match self.compression {
                    BlockCompression::Bc1 => decode_bc1_block(block),
                    BlockCompression::Bc3 => decode_bc3_block(block),
                };

                copy_block(
                    &mut rgba,
                    self.width as usize,
                    self.height as usize,
                    bx,
                    by,
                    &pixels,
                );
            }
        }

        Ok(rgba)
    }
}

fn expected_block_payload_len(width: u16, height: u16, mip_count: u8, block_bytes: usize) -> usize {
    let mut width = width;
    let mut height = height;
    let mut total = 0usize;

    for _ in 0..mip_count {
        total += block_level_len(width, height, block_bytes);
        width = (width / 2).max(1);
        height = (height / 2).max(1);
    }

    total
}

fn block_level_len(width: u16, height: u16, block_bytes: usize) -> usize {
    let blocks_x = (width as usize).div_ceil(4);
    let blocks_y = (height as usize).div_ceil(4);
    blocks_x.max(1) * blocks_y.max(1) * block_bytes
}

fn copy_block(
    out: &mut [u8],
    width: usize,
    height: usize,
    block_x: usize,
    block_y: usize,
    pixels: &[[u8; 4]; 16],
) {
    for py in 0..4 {
        for px in 0..4 {
            let x = block_x * 4 + px;
            let y = block_y * 4 + py;
            if x >= width || y >= height {
                continue;
            }

            let src = pixels[py * 4 + px];
            let dst = (y * width + x) * 4;
            out[dst..dst + 4].copy_from_slice(&src);
        }
    }
}

fn decode_bc1_block(block: &[u8]) -> [[u8; 4]; 16] {
    let color0 = u16::from_le_bytes([block[0], block[1]]);
    let color1 = u16::from_le_bytes([block[2], block[3]]);
    let mut palette = [[0u8; 4]; 4];
    palette[0] = rgb565(color0);
    palette[1] = rgb565(color1);

    if color0 > color1 {
        palette[2] = interpolate_rgba(palette[0], palette[1], 2, 1, 3);
        palette[3] = interpolate_rgba(palette[0], palette[1], 1, 2, 3);
    } else {
        palette[2] = interpolate_rgba(palette[0], palette[1], 1, 1, 2);
        palette[3] = [0, 0, 0, 0];
    }

    let indices = u32::from_le_bytes(block[4..8].try_into().unwrap());
    let mut out = [[0u8; 4]; 16];
    for (i, pixel) in out.iter_mut().enumerate() {
        let index = ((indices >> (2 * i)) & 0x3) as usize;
        *pixel = palette[index];
    }
    out
}

fn decode_bc3_block(block: &[u8]) -> [[u8; 4]; 16] {
    let alpha0 = block[0];
    let alpha1 = block[1];
    let alpha_palette = bc3_alpha_palette(alpha0, alpha1);

    let mut alpha_bits = 0u64;
    for i in 0..6 {
        alpha_bits |= (block[2 + i] as u64) << (8 * i);
    }

    let color0 = u16::from_le_bytes([block[8], block[9]]);
    let color1 = u16::from_le_bytes([block[10], block[11]]);
    let mut colors = [[0u8; 4]; 4];
    colors[0] = rgb565(color0);
    colors[1] = rgb565(color1);
    colors[2] = interpolate_rgba(colors[0], colors[1], 2, 1, 3);
    colors[3] = interpolate_rgba(colors[0], colors[1], 1, 2, 3);

    let color_bits = u32::from_le_bytes(block[12..16].try_into().unwrap());
    let mut out = [[0u8; 4]; 16];

    for (i, pixel) in out.iter_mut().enumerate() {
        let color_index = ((color_bits >> (2 * i)) & 0x3) as usize;
        let alpha_index = ((alpha_bits >> (3 * i)) & 0x7) as usize;
        *pixel = colors[color_index];
        pixel[3] = alpha_palette[alpha_index];
    }

    out
}

fn rgb565(value: u16) -> [u8; 4] {
    let r5 = ((value >> 11) & 0x1f) as u8;
    let g6 = ((value >> 5) & 0x3f) as u8;
    let b5 = (value & 0x1f) as u8;

    [
        ((r5 as u16 * 255 + 15) / 31) as u8,
        ((g6 as u16 * 255 + 31) / 63) as u8,
        ((b5 as u16 * 255 + 15) / 31) as u8,
        255,
    ]
}

fn interpolate_rgba(a: [u8; 4], b: [u8; 4], wa: u16, wb: u16, denom: u16) -> [u8; 4] {
    let mut out = [0u8; 4];
    for i in 0..4 {
        out[i] = ((a[i] as u16 * wa + b[i] as u16 * wb) / denom) as u8;
    }
    out
}

fn bc3_alpha_palette(a0: u8, a1: u8) -> [u8; 8] {
    let mut values = [0u8; 8];
    values[0] = a0;
    values[1] = a1;

    if a0 > a1 {
        for i in 1..=6 {
            values[i + 1] = (((7 - i) as u16 * a0 as u16 + i as u16 * a1 as u16) / 7) as u8;
        }
    } else {
        for i in 1..=4 {
            values[i + 1] = (((5 - i) as u16 * a0 as u16 + i as u16 * a1 as u16) / 5) as u8;
        }
        values[6] = 0;
        values[7] = 255;
    }

    values
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_bc3_texture() -> Vec<u8> {
        let mut bytes = vec![0u8; HEADER_LEN + 16 + FOOTER_LEN];
        let total_len = bytes.len() as u32;
        bytes[0..4].copy_from_slice(&2u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&total_len.to_le_bytes());
        bytes[12..20].copy_from_slice(MAGIC);
        bytes[32..36].copy_from_slice(b"TEST");
        bytes[40..42].copy_from_slice(&4u16.to_le_bytes());
        bytes[42..44].copy_from_slice(&4u16.to_le_bytes());
        bytes[46] = 1;
        bytes[47] = 1;

        let block = &mut bytes[HEADER_LEN..HEADER_LEN + 16];
        block[0] = 255;
        block[1] = 0;
        block[8..10].copy_from_slice(&0xf800u16.to_le_bytes());
        block[10..12].copy_from_slice(&0u16.to_le_bytes());
        bytes
    }

    #[test]
    fn parses_known_layout_and_infers_bc3() {
        let bytes = synthetic_bc3_texture();
        let texture = Texture2Db::parse(&bytes).unwrap();
        assert_eq!(texture.name, "TEST");
        assert_eq!((texture.width, texture.height), (4, 4));
        assert_eq!(texture.mip_count, 1);
        assert_eq!(texture.compression, BlockCompression::Bc3);
        assert_eq!(texture.base_mip_len(), 16);
    }

    #[test]
    fn decodes_simple_bc3_block() {
        let bytes = synthetic_bc3_texture();
        let texture = Texture2Db::parse(&bytes).unwrap();
        let rgba = texture.decode_base_rgba8().unwrap();

        assert_eq!(rgba.len(), 4 * 4 * 4);
        for pixel in rgba.chunks_exact(4) {
            assert_eq!(pixel, [255, 0, 0, 255]);
        }
    }

    #[test]
    fn known_tdu_sizes_match_bc3_mip_chains() {
        assert_eq!(expected_block_payload_len(1024, 256, 7, 16), 349_504);
        assert_eq!(expected_block_payload_len(1024, 128, 6, 16), 174_720);
    }
}
