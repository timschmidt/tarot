//! GPU copies of the worker's bounded rasters. No file access or decoding here.
use egui::{Color32, Context, Painter, Pos2, Rect, TextureHandle};
use std::{collections::BTreeMap, sync::Arc};
use tarot_core::{Command, ShellModel, icons::Raster};

type Textures = BTreeMap<String, (Arc<Raster>, TextureHandle)>;
fn key() -> egui::Id {
    egui::Id::new("tarot_native_icon_textures")
}

pub fn prune(ctx: &Context, model: &ShellModel) {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Textures>(key())
            .retain(|id, (raster, _)| {
                model
                    .icons
                    .image(id)
                    .is_some_and(|r| Arc::ptr_eq(r, raster))
            });
    });
}
pub fn request(model: &ShellModel, id: &str, commands: &mut Vec<Command>) {
    if model.icons.needs_load(id)
        && model.applications.iter().any(|a| {
            a.id == id
                && a.icon
                    .as_ref()
                    .is_some_and(|s| !s.is_empty() && s.len() <= 4096)
        })
        && !commands
            .iter()
            .any(|c| matches!(c,Command::LoadIcon(i) if i==id))
    {
        commands.push(Command::LoadIcon(id.into()));
    }
}
pub fn paint(
    ctx: &Context,
    painter: &Painter,
    model: &ShellModel,
    id: &str,
    rect: Rect,
    tint: Color32,
) -> bool {
    let Some(raster) = model.icons.image(id) else {
        return false;
    };
    let existing = ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Textures>(key())
            .get(id)
            .filter(|(r, _)| Arc::ptr_eq(r, raster))
            .map(|(_, t)| t.id())
    });
    let texture = existing.unwrap_or_else(|| {
        let texture = ctx.load_texture(
            "native application icon",
            egui::ColorImage::from_rgba_unmultiplied(raster.size(), raster.rgba()),
            egui::TextureOptions::LINEAR,
        );
        let id_texture = texture.id();
        ctx.data_mut(|d| {
            d.get_temp_mut_or_default::<Textures>(key())
                .insert(id.into(), (raster.clone(), texture));
        });
        id_texture
    });
    painter.image(
        texture,
        rect,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        tint,
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn late_raster_upload_is_reused_and_eviction_frees_texture() {
        let ctx = Context::default();
        let mut model = ShellModel::default();
        model.applications.push(tarot_core::Application {
            id: "custom.desktop".into(),
            name: "Custom".into(),
            description: String::new(),
            icon: Some("custom".into()),
            keywords: vec![],
            unavailable: None,
        });
        let mut commands = vec![];
        request(&model, "custom.desktop", &mut commands);
        request(&model, "custom.desktop", &mut commands);
        assert_eq!(commands, vec![Command::LoadIcon("custom.desktop".into())]);
        assert_eq!(
            model.apply(commands.remove(0)),
            vec![tarot_core::Effect::LoadIcon {
                id: "custom.desktop".into(),
                icon: "custom".into()
            }]
        );
        request(&model, "custom.desktop", &mut commands);
        assert!(commands.is_empty());
        let rect = Rect::from_min_size(Pos2::ZERO, egui::Vec2::splat(58.0));
        let painter = ctx.layer_painter(egui::LayerId::background());
        assert!(!paint(
            &ctx,
            &painter,
            &model,
            "custom.desktop",
            rect,
            Color32::WHITE
        ));
        model.icons.complete(
            "custom.desktop".into(),
            Raster::new([1, 1], vec![12, 34, 56, 128]),
        );
        assert!(paint(
            &ctx,
            &painter,
            &model,
            "custom.desktop",
            rect,
            Color32::WHITE
        ));
        let texture = ctx.data_mut(|d| {
            d.get_temp_mut_or_default::<Textures>(key())["custom.desktop"]
                .1
                .id()
        });
        assert!(paint(
            &ctx,
            &painter,
            &model,
            "custom.desktop",
            rect,
            Color32::WHITE
        ));
        assert_eq!(
            texture,
            ctx.data_mut(
                |d| d.get_temp_mut_or_default::<Textures>(key())["custom.desktop"]
                    .1
                    .id()
            )
        );
        model.icons = Default::default();
        prune(&ctx, &model);
        assert!(ctx.data_mut(|d| d.get_temp_mut_or_default::<Textures>(key()).is_empty()));
        model.system_menu_open = true;
        model.notifications_open = true;
        model.shell_text_focus = true;
        assert!(
            model
                .apply(Command::LoadIcon("unknown.desktop".into()))
                .is_empty()
        );
        assert!(model.system_menu_open && model.notifications_open && model.shell_text_focus);
    }
}
