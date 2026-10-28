//! Runtime use of the owner's original Nocturne packet; source art stays intact.
use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, TextureHandle, Vec2};
use std::{collections::BTreeMap, io::Cursor};

pub const STATUS: Color32 = Color32::from_rgb(3, 5, 5);
pub const GESTURE: Color32 = Color32::from_rgb(7, 13, 16);
pub const TEXT: Color32 = Color32::from_rgb(244, 246, 239);
pub const FOCUS: Color32 = Color32::from_rgb(215, 190, 137);

pub fn bold(ctx: &egui::Context, size: f32) -> FontId {
    let family = FontFamily::Name("tarot-bold".into());
    // set_fonts is consumed at the next frame boundary. Direct users of the
    // shared view can still draw their first frame before that boundary.
    if ctx.fonts(|f| f.families().contains(&family)) {
        FontId::new(size, family)
    } else {
        FontId::proportional(size)
    }
}

pub fn install(ctx: &egui::Context) {
    let id = egui::Id::new("tarot_nocturne_fonts");
    if ctx.data_mut(|d| d.get_temp::<bool>(id)).unwrap_or(false) {
        return;
    }
    let mut fonts = FontDefinitions::default();
    for (name, data) in [
        (
            "tarot-lato",
            include_bytes!("../assets/fonts/Lato-Regular.ttf").as_slice(),
        ),
        (
            "tarot-bold",
            include_bytes!("../assets/fonts/Lato-Bold.ttf").as_slice(),
        ),
        (
            "tarot-italic",
            include_bytes!("../assets/fonts/Lato-Italic.ttf").as_slice(),
        ),
        (
            "tarot-brand",
            include_bytes!("../assets/fonts/InstrumentSerif-Regular.ttf").as_slice(),
        ),
    ] {
        fonts
            .font_data
            .insert(name.into(), FontData::from_static(data).into());
        if name != "tarot-lato" {
            let mut family = vec![name.to_owned()];
            family.extend(fonts.families[&FontFamily::Proportional].clone());
            fonts.families.insert(FontFamily::Name(name.into()), family);
        }
    }
    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "tarot-lato".into());
    ctx.set_fonts(fonts);
    ctx.style_mut(|s| {
        s.text_styles
            .insert(egui::TextStyle::Heading, FontId::proportional(24.0));
        s.text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(19.0));
        s.text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(17.0));
        s.text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(13.0));
        s.visuals.selection.bg_fill = FOCUS.gamma_multiply(0.3);
        s.visuals.selection.stroke.color = FOCUS;
    });
    ctx.data_mut(|d| d.insert_temp(id, true));
}

#[derive(Clone)]
struct Art {
    wallpaper: TextureHandle,
    tray: TextureHandle,
    icons: BTreeMap<&'static str, TextureHandle>,
}

fn decode(bytes: &[u8]) -> egui::ColorImage {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().expect("valid bundled design PNG");
    let mut pixels = vec![0; reader.output_buffer_size().expect("bounded design PNG")];
    let info = reader
        .next_frame(&mut pixels)
        .expect("complete bundled design PNG");
    assert!(info.width <= 4096 && info.height <= 4096);
    let size = [info.width as usize, info.height as usize];
    match info.color_type {
        png::ColorType::Rgba => {
            egui::ColorImage::from_rgba_unmultiplied(size, &pixels[..info.buffer_size()])
        }
        png::ColorType::Rgb => egui::ColorImage::from_rgb(size, &pixels[..info.buffer_size()]),
        _ => panic!("bundled artwork must be RGB or RGBA"),
    }
}

pub fn prepare_art(ctx: &egui::Context) {
    let key = egui::Id::new("tarot_original_nocturne_art");
    if ctx.data_mut(|d| d.get_temp::<Art>(key)).is_some() {
        return;
    }
    let wallpaper = ctx.load_texture(
        "Nocturne",
        decode(include_bytes!("../assets/nocturne.png")),
        egui::TextureOptions::LINEAR,
    );
    let tray = ctx.load_texture(
        "Nocturne Quick Launch tray",
        decode(include_bytes!("../assets/tray.png")),
        egui::TextureOptions::LINEAR,
    );
    let mut icons = BTreeMap::new();
    macro_rules! icon {
        ($name:literal) => {
            icon!($name, $name)
        };
        ($name:literal, $asset:literal) => {
            icons.insert(
                $name,
                ctx.load_texture(
                    concat!("Tarot ", $name),
                    decode(include_bytes!(concat!("../assets/icons/", $asset, ".png"))),
                    egui::TextureOptions::LINEAR,
                ),
            );
        };
    }
    icon!("phone");
    icon!("contacts");
    icon!("mail");
    icon!("calendar", "calendar-base");
    icon!("launcher");
    icon!("web");
    icon!("store");
    icon!("messages");
    icon!("camera");
    icon!("photos");
    icon!("music");
    icon!("clock");
    icon!("notes");
    icon!("settings");
    icon!("files");
    icon!("terminal");
    ctx.data_mut(|d| {
        d.insert_temp(
            key,
            Art {
                wallpaper,
                tray,
                icons,
            },
        )
    });
}

pub fn wallpaper(ctx: &egui::Context, layout: tarot_core::Layout) -> egui::Shape {
    prepare_art(ctx);
    let id = ctx
        .data_mut(|d| d.get_temp::<Art>(egui::Id::new("tarot_original_nocturne_art")))
        .unwrap()
        .wallpaper
        .id();
    let mut mesh = egui::Mesh::with_texture(id);
    let extent = Vec2::new(layout.width, layout.height);
    // Cover without distorting the original portrait artwork in landscape.
    let aspect = layout.width / layout.height.max(1.0);
    let uv = if aspect > 0.5 {
        let height = 0.5 / aspect;
        egui::Rect::from_min_max(
            egui::pos2(0.0, (1.0 - height) / 2.0),
            egui::pos2(1.0, (1.0 + height) / 2.0),
        )
    } else {
        let width = aspect / 0.5;
        egui::Rect::from_min_max(
            egui::pos2((1.0 - width) / 2.0, 0.0),
            egui::pos2((1.0 + width) / 2.0, 1.0),
        )
    };
    mesh.add_rect_with_uv(
        egui::Rect::from_min_size(egui::Pos2::ZERO, extent),
        uv,
        Color32::WHITE,
    );
    egui::Shape::mesh(mesh)
}

fn icon(ctx: &egui::Context, id: &str, launcher: bool) -> Option<egui::TextureId> {
    let id = id.to_ascii_lowercase();
    let name = if launcher {
        "launcher"
    } else if id.contains("calls") || id.contains("dialer") {
        "phone"
    } else if id.contains("contacts") || id.contains("phonebook") {
        "contacts"
    } else if id.contains("geary") || id.contains("mail") {
        "mail"
    } else if id.contains("calendar") || id.contains("kalendar") {
        "calendar"
    } else if id.contains("epiphany") || id.contains("firefox") || id.contains("browser") {
        "web"
    } else if id.contains("chatty") || id.contains("message") {
        "messages"
    } else if id.contains("camera") || id.contains("snapshot") {
        "camera"
    } else if id.contains("photo") || id.contains("loupe") {
        "photos"
    } else if id.contains("music") || id.contains("lollypop") {
        "music"
    } else if id.contains("clock") {
        "clock"
    } else if id.contains("notes") || id.contains("memo") {
        "notes"
    } else if id.contains("settings") || id.contains("control-center") {
        "settings"
    } else if id.contains("nautilus") || id.contains("files") {
        "files"
    } else if id.contains("terminal") || id == "foot.desktop" || id.contains("console") {
        "terminal"
    } else if id.contains("software") || id.contains("store") {
        "store"
    } else {
        return None;
    };
    ctx.data_mut(|d| d.get_temp::<Art>(egui::Id::new("tarot_original_nocturne_art")))?
        .icons
        .get(name)
        .map(|t| t.id())
}

/// Original translucent material; the wave deforms its mesh with the targets.
pub fn tray(ctx: &egui::Context) -> egui::TextureId {
    prepare_art(ctx);
    ctx.data_mut(|d| d.get_temp::<Art>(egui::Id::new("tarot_original_nocturne_art")))
        .unwrap()
        .tray
        .id()
}

/// Draw original role artwork, with a live Calendar day in the packet's pose.
pub fn paint_icon(
    ctx: &egui::Context,
    painter: &egui::Painter,
    id: &str,
    launcher: bool,
    rect: egui::Rect,
    tint: Color32,
    day: Option<u8>,
) -> bool {
    let Some(texture) = icon(ctx, id, launcher) else {
        return false;
    };
    painter.image(
        texture,
        rect,
        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
        tint,
    );
    let calendar = !launcher && {
        let id = id.to_ascii_lowercase();
        id.contains("calendar") || id.contains("kalendar")
    };
    if calendar && let Some(day) = day.filter(|d| (1..=31).contains(d)) {
        let scale = rect.width() / 64.0;
        let color = Color32::from_rgb(32, 40, 43).gamma_multiply(f32::from(tint.r()) / 255.0);
        let text = painter.layout_no_wrap(day.to_string(), bold(ctx, 30.0 * scale), color);
        let center = rect.center() + egui::vec2(0.0, 7.0 * scale);
        painter.add(
            egui::epaint::TextShape::new(center - text.size() * 0.5, text, color)
                .with_angle_and_anchor(3.0_f32.to_radians(), egui::Align2::CENTER_CENTER),
        );
    }
    true
}
