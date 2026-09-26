//! Page registry. Each entry mirrors a Vue Memoria screen — see PARITY.md for
//! the `src/` source it must match and the milestone that implements it.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    /// «Всё» — Everything grid of notes and objects.
    Everything,
    /// Note editor (tiptap parity surface).
    Note,
    /// «Пузырьковый дневник» — bubble diary timeline.
    Diary,
    /// Books / typed object collections.
    Books,
    /// «Настройки» — general, export, trash sections.
    Settings,
    /// «Корзина» — soft-deleted entries.
    Trash,
    /// Floating sticker note window.
    Sticker,
}

impl Page {
    pub const ALL: &[Page] = &[
        Page::Everything,
        Page::Note,
        Page::Diary,
        Page::Books,
        Page::Settings,
        Page::Trash,
        Page::Sticker,
    ];

    /// Russian title shown in navigation; matches Vue Memoria labels.
    pub fn title(self) -> &'static str {
        match self {
            Page::Everything => "Всё",
            Page::Note => "Заметка",
            Page::Diary => "Дневник",
            Page::Books => "Книги",
            Page::Settings => "Настройки",
            Page::Trash => "Корзина",
            Page::Sticker => "Стикер",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_russian_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for page in Page::ALL {
            let title = page.title();
            assert!(!title.is_empty());
            assert!(seen.insert(title), "duplicate title {title}");
        }
    }
}
