//! What an instance's icon shows (plan R8): its own picture (one the player chose, or its
//! modpack's icon) first; else the block or colour chosen in its dialog; else its mark: the loader
//! it plays with, the grass block for plain Minecraft, Horizon's own for Horizon. The letter tile
//! is the last resort. Each is decoded once per size and kept in `Instances::icons`.
use super::*;
use gpui::RenderImage;
use horizon_launcher::browser::images;
use std::sync::Arc;

/// Quilt's mini icon (CC0) and NeoForge's (CC BY 4.0), from their own art repositories: see
/// THIRD_PARTY_NOTICES.md.
const QUILT: &[u8] = include_bytes!("../assets/logos/quilt.png");
const NEOFORGE: &[u8] = include_bytes!("../assets/logos/neoforge.png");
/// Fabric and Forge publish no licence for their logos: their own icons, fetched from their own
/// sites into the image cache, never shipped.
const FABRIC: &str = "https://raw.githubusercontent.com/FabricMC/fabric-loader/master/src/main/resources/assets/fabricloader/icon.png";
const FORGE: &str = "https://files.minecraftforge.net/static/images/favicon-32x32.png";
pub(super) const GRASS: &[u8] = include_bytes!("../assets/logos/grass-block.png");
impl Launcher {
    pub(super) fn grass_mark(&mut self, size: f32) -> Option<Arc<RenderImage>> {
        self.icon_image("mark:grass".into(), size, false, || Some(GRASS.to_vec()))
    }

    /// A mod loader's logo by its id (`fabric`, `quilt`, `forge`, `neoforge`): None while a
    /// fetched one is on its way, or for another id.
    pub(super) fn loader_logo(&mut self, loader: &str, size: f32) -> Option<Arc<RenderImage>> {
        let bundled = |png: &'static [u8]| move || Some(png.to_vec());
        let key = format!("mark:{loader}");
        match loader {
            "quilt" => self.icon_image(key, size, false, bundled(QUILT)),
            "neoforge" => self.icon_image(key, size, false, bundled(NEOFORGE)),
            "fabric" | "forge" => {
                let url = if loader == "fabric" { FABRIC } else { FORGE };
                let file = self.mods_image(url, images::ICON)?;
                self.icon_image(key, size, false, || std::fs::read(file).ok())
            }
            _ => None,
        }
    }

    /// The image under `key` at `size` points, made from `bytes` the first time. A missing one is
    /// remembered only when `remember_missing` (a file that may never exist); else it is asked
    /// again next frame (a logo still downloading).
    fn icon_image(
        &mut self,
        key: String,
        size: f32,
        remember_missing: bool,
        bytes: impl FnOnce() -> Option<Vec<u8>>,
    ) -> Option<Arc<RenderImage>> {
        let key = (key, (size * self.scale) as u32);
        if let Some(image) = self.instances.icons.get(&key) {
            return image.clone();
        }
        let image = bytes().and_then(|b| fitted(b, size, self.scale));
        if image.is_some() || remember_missing {
            self.instances.icons.insert(key, image.clone());
        }
        image
    }
}

/// An image file's bytes drawn at most `size` points square: scaled down smoothly when larger,
/// up by whole device pixels when smaller, so pixel-art logos stay sharp.
fn fitted(bytes: Vec<u8>, size: f32, scale: f32) -> Option<Arc<RenderImage>> {
    let side = (size * scale).round().max(1.) as u32;
    let png = images::fit_png(bytes, side, 4096, 64 << 20)?;
    let image = image::load_from_memory(&png).ok()?.to_rgba8();
    let (w, h) = (image.width() as usize, image.height() as usize);
    let texels: Vec<[u8; 4]> = image.pixels().map(|p| p.0).collect();
    let whole = (side as usize / w.max(h)).max(1) as f32;
    Some(art::bake(&texels, w, h, whole / scale, scale))
}

/// An image made by [`fitted`], centred in a `size` points square.
pub(super) fn centred(image: Arc<RenderImage>, size: f32, scale: f32) -> Div {
    let dim = image.size(0);
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .child(
            gpui::img(image)
                .w(px(dim.width.0 as f32 / scale))
                .h(px(dim.height.0 as f32 / scale)),
        )
}
