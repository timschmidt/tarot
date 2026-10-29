use resvg::{tiny_skia as sk, usvg};
use std::{
    io::Cursor,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use tarot_core::icons::Raster;

const MAX_FILE: usize = 1024 * 1024;
const MAX_SOURCE_PIXELS: u64 = 4 * 1024 * 1024;

pub struct Decoder {
    fonts: std::sync::Arc<usvg::fontdb::Database>,
}
impl Decoder {
    pub fn new() -> Self {
        let mut fonts = usvg::fontdb::Database::new();
        fonts.load_font_data(
            include_bytes!("../../crates/tarot-ui/assets/fonts/Lato-Regular.ttf").to_vec(),
        );
        fonts.set_sans_serif_family("Lato");
        Self {
            fonts: std::sync::Arc::new(fonts),
        }
    }
    pub fn load(&self, path: &Path, pixels: u32) -> Option<Raster> {
        if !(1..=256).contains(&pixels) {
            return None;
        }
        let bytes = super::read(path, MAX_FILE)?;
        match path.extension()?.to_str()? {
            "png" => self.png(&bytes, pixels),
            "svg" => self.svg(&bytes, pixels),
            _ => None,
        }
    }
    fn svg(&self, bytes: &[u8], pixels: u32) -> Option<Raster> {
        // No compressed inputs, image imports (including data URLs), remote
        // resources, or system font scanning. Unsupported imports use fallback.
        if bytes.len() > MAX_FILE || bytes.iter().filter(|b| **b == b'<').count() > 10_000 {
            return None;
        }
        let rejected = AtomicBool::new(false);
        let options = usvg::Options {
            font_family: "Lato".into(),
            fontdb: self.fonts.clone(),
            image_href_resolver: usvg::ImageHrefResolver {
                resolve_string: Box::new(|_, _| {
                    rejected.store(true, Ordering::Relaxed);
                    None
                }),
                resolve_data: Box::new(|_, _, _| {
                    rejected.store(true, Ordering::Relaxed);
                    None
                }),
            },
            ..Default::default()
        };
        let tree = usvg::Tree::from_data(bytes, &options).ok()?;
        if rejected.load(Ordering::Relaxed) {
            return None;
        }
        let mut output = sk::Pixmap::new(pixels, pixels)?;
        let size = tree.size();
        let scale = (pixels as f32 / size.width()).min(pixels as f32 / size.height());
        let transform = sk::Transform::from_translate(
            (pixels as f32 - size.width() * scale) * 0.5,
            (pixels as f32 - size.height() * scale) * 0.5,
        )
        .pre_scale(scale, scale);
        resvg::render(&tree, transform, &mut output.as_mut());
        raster(output)
    }
    fn png(&self, bytes: &[u8], pixels: u32) -> Option<Raster> {
        let mut decoder = png::Decoder::new_with_limits(
            Cursor::new(bytes),
            png::Limits {
                bytes: 32 * 1024 * 1024,
            },
        );
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().ok()?;
        let info = reader.info();
        if info.width == 0
            || info.height == 0
            || u64::from(info.width) * u64::from(info.height) > MAX_SOURCE_PIXELS
        {
            return None;
        }
        let mut data = vec![0; reader.output_buffer_size()?];
        let info = reader.next_frame(&mut data).ok()?;
        let mut rgba = Vec::with_capacity(info.width as usize * info.height as usize * 4);
        let channels = match info.color_type {
            png::ColorType::Rgb => 3,
            png::ColorType::Rgba => 4,
            png::ColorType::Grayscale => 1,
            png::ColorType::GrayscaleAlpha => 2,
            png::ColorType::Indexed => return None,
        };
        for pixel in data[..info.buffer_size()].chunks_exact(channels) {
            let (r, g, b, a) = match channels {
                1 => (pixel[0], pixel[0], pixel[0], 255),
                2 => (pixel[0], pixel[0], pixel[0], pixel[1]),
                3 => (pixel[0], pixel[1], pixel[2], 255),
                _ => (pixel[0], pixel[1], pixel[2], pixel[3]),
            };
            for channel in [r, g, b] {
                rgba.push(((u32::from(channel) * u32::from(a) + 127) / 255) as u8);
            }
            rgba.push(a);
        }
        let source = sk::Pixmap::from_vec(rgba, sk::IntSize::from_wh(info.width, info.height)?)?;
        let mut output = sk::Pixmap::new(pixels, pixels)?;
        let scale = (pixels as f32 / info.width as f32).min(pixels as f32 / info.height as f32);
        output.draw_pixmap(
            0,
            0,
            source.as_ref(),
            &sk::PixmapPaint {
                quality: sk::FilterQuality::Bilinear,
                ..Default::default()
            },
            sk::Transform::from_translate(
                (pixels as f32 - info.width as f32 * scale) * 0.5,
                (pixels as f32 - info.height as f32 * scale) * 0.5,
            )
            .pre_scale(scale, scale),
            None,
        );
        raster(output)
    }
}
fn raster(pixmap: sk::Pixmap) -> Option<Raster> {
    let size = [pixmap.width() as usize, pixmap.height() as usize];
    let mut rgba = Vec::with_capacity(size[0] * size[1] * 4);
    for pixel in pixmap.pixels() {
        let c = pixel.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    if !rgba.as_chunks::<4>().0.iter().any(|p| p[3] != 0) {
        return None;
    }
    Raster::new(size, rgba)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn svg_aspect_alpha_and_imports() {
        let decoder = Decoder::new();
        let image = decoder.svg(br##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="#ff0000" fill-opacity="0.5"/></svg>"##,40).unwrap();
        assert_eq!(image.size(), [40, 40]);
        assert_eq!(&image.rgba()[..4], &[0, 0, 0, 0]);
        let p = (20 * 40 + 20) * 4;
        assert_eq!(&image.rgba()[p..p + 4], &[255, 0, 0, 128]);
        for href in [
            "/tmp/private.png",
            "https://example.org/image.png",
            "data:image/png;base64,aGVsbG8=",
        ] {
            let svg = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><rect width="20" height="20"/><image href="{href}" width="20" height="20"/></svg>"#
            );
            assert!(decoder.svg(svg.as_bytes(), 40).is_none());
        }
        assert!(decoder.svg(b"invalid", 40).is_none());
    }
    #[test]
    fn png_expansion_and_size_rejection() {
        let decoder = Decoder::new();
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
            encoder.set_color(png::ColorType::GrayscaleAlpha);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&[200, 128, 200, 128])
                .unwrap();
        }
        let image = decoder.png(&bytes, 4).unwrap();
        assert_eq!(image.size(), [4, 4]);
        assert_eq!(&image.rgba()[..4], &[0, 0, 0, 0]);
        let p = (2 * 4 + 2) * 4;
        assert!((i16::from(image.rgba()[p]) - 200).abs() <= 1);
        assert_eq!(image.rgba()[p + 3], 128);
        let mut huge = Vec::new();
        png::Encoder::new(&mut huge, 100_000, 100_000)
            .write_header()
            .unwrap()
            .finish()
            .unwrap();
        assert!(decoder.png(&huge, 4).is_none());
        assert!(decoder.png(b"invalid", 4).is_none());
    }
}
