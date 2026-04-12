//! Layout-related message handlers.

use iced::Task;

use crate::app::message::{ContentWidthTarget, Message};
use crate::app::state::App;

const GRID_PADDING: f32 = 64.0;
const DETAIL_GRID_PADDING: f32 = 96.0;

impl App {
    /// Handle measured content width updates from responsive pages.
    pub fn handle_layout(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::ContentWidthResized(target, size) => {
                let width = match target {
                    ContentWidthTarget::Search | ContentWidthTarget::Discover => {
                        crate::ui::widgets::usable_content_width(*size, GRID_PADDING)
                    }
                    ContentWidthTarget::PlaylistDetail => {
                        crate::ui::widgets::usable_content_width(*size, DETAIL_GRID_PADDING)
                    }
                };

                match target {
                    ContentWidthTarget::Search => self.ui.search.content_width = width,
                    ContentWidthTarget::Discover => self.ui.discover.content_width = width,
                    ContentWidthTarget::PlaylistDetail => {
                        self.ui.playlist_page.content_width = width;
                    }
                }

                Some(Task::none())
            }

            _ => None,
        }
    }
}
