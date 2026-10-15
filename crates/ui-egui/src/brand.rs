//! ArtCraft wordmarks are trademarks of the ArtCraft Team and are not included in this fork.
//!
//! `docs/brand/LICENSE-brand.txt` requires forks and modified versions to remove the marks.
//! The About dialog and Home screen simply skip the wordmark; FilmCraft attribution stays in
//! plain text (`NOTICE`).

/// This fork has no wordmark texture.
pub fn wordmark(_ctx: &egui::Context, _dark: bool) -> Option<egui::TextureHandle> {
    None
}

/// Draw nothing. Returns `None` so callers keep the rest of the layout.
pub fn paint_wordmark(_ui: &egui::Ui, _left_center: egui::Pos2, _height: f32, _dark: bool) -> Option<egui::Rect> {
    None
}
