//! Diary bubble + app-network command arms for Worker (source-size split).
use super::app_network_api;
use super::bubble_api::{self, BubbleApi};
use super::entry_api::EntryApi;
use super::transport::Engine;
use super::{err_string, Command, Reply};

pub(super) fn dispatch_bubble_or_network(
    command: Command,
    api: &mut EntryApi<Engine>,
    bubbles: &BubbleApi<Engine>,
) -> Option<Reply> {
    Some(match command {
        Command::ListBubbles => Reply::Bubbles(bubbles.list_bubbles().map_err(err_string)),
        Command::CreateBubble {
            input,
            kind,
            parent_id,
            content_json,
        } => Reply::BubbleCreated(
            bubbles
                .create_bubble(&input, kind, parent_id.as_deref(), content_json)
                .map_err(err_string),
        ),
        Command::UpdateBubble { id, patch } => Reply::BubbleUpdated {
            result: bubbles.update_bubble(&id, patch).map_err(err_string),
            id,
        },
        Command::DeleteBubble(id) => Reply::BubbleDeleted {
            result: bubbles.delete_bubble(&id).map_err(err_string),
            id,
        },
        Command::MigrateBubble {
            namespace,
            source_id,
            bubble,
        } => Reply::BubbleMigrated {
            result: bubbles
                .migrate_bubble(&namespace, &source_id, &bubble)
                .map_err(err_string),
            source_id,
        },
        Command::MigrateDiary { local_bubbles_json } => Reply::DiaryMigrated(
            bubble_api::migrate_diary(bubbles, api, local_bubbles_json.as_ref())
                .map_err(err_string),
        ),
        Command::LookupIsbn(isbn) => {
            let result = app_network_api::lookup_isbn(api.bridge(), &isbn).map_err(err_string);
            Reply::BookMetadata(result)
        }
        Command::FetchBookPage(url) => {
            let result = app_network_api::fetch_book_page(api.bridge(), &url).map_err(err_string);
            Reply::BookMetadataPage(result)
        }
        Command::DominantColor(source) => {
            let result = app_network_api::dominant_color(api.bridge(), &source).map_err(err_string);
            Reply::DominantColor { source, result }
        }
        Command::StoreCover {
            source_path,
            entry_id,
        } => {
            let result = app_network_api::store_cover(api.bridge(), &source_path, &entry_id)
                .map_err(err_string);
            Reply::CoverStored { entry_id, result }
        }
        Command::FetchImage(url) => {
            let result = app_network_api::fetch_image(api.bridge(), &url).map_err(err_string);
            Reply::ImageFetched { url, result }
        }
        _ => return None,
    })
}
