// Re-exported from imago-gpui: the shared Kosmos GPUI visual core owns the
// token table. `pal()`, `BG()`/`FG()`/…, `c`/`rgba`/`mix`/`lerp` come from
// there; `imago_gpui::theme::apply` installs the palette as the
// gpui-component theme.
pub use imago_gpui::theme::*;
