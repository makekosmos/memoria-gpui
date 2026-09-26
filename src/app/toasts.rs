//! `ToastHost` port — bottom-right toast stack with auto-dismiss (TTL handled
//! in `Memoria::toast`).
use gpui::{div, prelude::*, px, Context, SharedString};
use gpui_component::Sizable;

use super::Memoria;
use crate::theme::*;

/// `IconId` → `gpui::assets::IconName` (full bundled Lucide catalog).
pub(crate) fn icon_name(id: memoria_gpui::sidebar_model::IconId) -> gpui::assets::IconName {
    use gpui::assets::IconName as N;
    use memoria_gpui::sidebar_model::IconId as I;
    match id {
        I::File => N::File,
        I::FileText => N::FileText,
        I::Gamepad2 => N::Gamepad2,
        I::Image => N::Image,
        I::Dumbbell => N::Dumbbell,
        I::Activity => N::Activity,
        I::BookOpen => N::BookOpen,
        I::Calendar => N::Calendar,
        I::Orbit => N::Orbit,
        I::Library => N::Library,
        I::Folder => N::Folder,
        I::Sparkles => N::Sparkles,
        I::User => N::User,
        I::Pin => N::Pin,
        I::Search => N::Search,
        I::Settings => N::Settings,
        I::Trash => N::Trash,
        I::LayoutGrid => N::LayoutGrid,
        I::NotebookPen => N::NotebookPen,
        I::ChevronLeft => N::ChevronLeft,
        I::ChevronRight => N::ChevronRight,
        I::Plus => N::Plus,
        I::X => N::X,
        I::EllipsisVertical => N::EllipsisVertical,
        I::RotateCcw => N::RotateCcw,
        I::Check => N::Check,
        I::StickyNote => N::StickyNote,
        I::Minus => N::Minus,
        I::Square => N::Square,
        I::Maximize2 => N::Maximize2,
        I::Minimize2 => N::Minimize2,
        I::Copy => N::Copy,
        I::RefreshCw => N::RefreshCw,
        I::ExternalLink => N::ExternalLink,
        I::House => N::House,
        I::Tag => N::Tag,
        I::Boxes => N::Boxes,
    }
}

pub(crate) fn icon(
    id: memoria_gpui::sidebar_model::IconId,
    size: f32,
    color: gpui::Hsla,
) -> gpui_component::Icon {
    gpui_component::Icon::new(icon_name(id))
        .with_size(px(size))
        .text_color(color)
}

impl Memoria {
    /// Toast stack — fixed bottom-right, newest last (ToastHost.vue).
    pub(crate) fn render_toasts(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut host = div()
            .id("toast-host")
            .debug_selector(|| "toast-host".into())
            .absolute()
            .right_4()
            .bottom_4()
            .flex()
            .flex_col()
            .gap_2()
            .items_end();
        for toast in &self.toasts {
            host = host.child(
                div()
                    .id(SharedString::from(format!("toast-{}", toast.id)))
                    .debug_selector(|| "toast".into())
                    .px_4()
                    .py_2()
                    .rounded_lg()
                    .bg(c(SIDEBAR_BG()))
                    .border_1()
                    .border_color(c(BORDER()))
                    .shadow_lg()
                    .text_size(px(13.))
                    .text_color(c(FG()))
                    .child(toast.text.clone()),
            );
        }
        host
    }
}
