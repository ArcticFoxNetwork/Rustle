// src/app/update/window.rs
//! Window and exit dialog message handlers

use iced::Task;
use iced::time::Instant;

use crate::app::message::Message;
use crate::app::state::App;
use crate::features::CloseBehavior;
use crate::platform::window;

impl App {
    /// Handle window-related messages
    pub fn handle_window(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::RequestClose => {
                match self.core.settings.close_behavior {
                    CloseBehavior::Ask => {
                        self.ui.dialogs.exit_open = true;
                        self.ui.dialogs.exit_animation.start(Instant::now());
                    }
                    CloseBehavior::Exit => {
                        return Some(iced::exit());
                    }
                    CloseBehavior::MinimizeToTray => {
                        tracing::info!("Hiding window to tray");
                        self.core.window_hidden = true;
                        return Some(window::hide_window());
                    }
                }
                Some(Task::none())
            }

            Message::ConfirmExit => {
                if self.ui.dialogs.exit_remember {
                    self.core.settings.close_behavior = CloseBehavior::Exit;
                    let _ = self.core.settings.save();
                }
                Some(iced::exit())
            }

            Message::MinimizeToTray => {
                if self.ui.dialogs.exit_remember {
                    self.core.settings.close_behavior = CloseBehavior::MinimizeToTray;
                    let _ = self.core.settings.save();
                }
                self.ui.dialogs.exit_open = false;
                self.ui.dialogs.exit_animation.stop(Instant::now());
                tracing::info!("Hiding window to tray");
                self.core.window_hidden = true;
                Some(window::hide_window())
            }

            Message::CancelExit => {
                self.ui.dialogs.exit_open = false;
                self.ui.dialogs.exit_animation.stop(Instant::now());
                Some(Task::none())
            }

            Message::ExitDialogRememberChanged(checked) => {
                self.ui.dialogs.exit_remember = *checked;
                Some(Task::none())
            }

            Message::ToggleWindow => {
                self.core.window_hidden = !self.core.window_hidden;
                let visible = !self.core.window_hidden;
                tracing::info!("Setting window visible: {}", visible);

                if visible {
                    Some(window::show_window())
                } else {
                    Some(window::hide_window())
                }
            }

            Message::ShowWindow => {
                self.core.window_hidden = false;
                tracing::info!("Showing window");
                Some(window::show_window())
            }

            Message::WindowOperationComplete => {
                self.core.window_operation_pending = false;
                Some(Task::none())
            }

            // Sidebar resize
            Message::SidebarResizeStart => {
                self.ui.sidebar_dragging = true;
                Some(Task::none())
            }

            Message::SidebarResizeEnd => {
                self.ui.sidebar_dragging = false;
                Some(Task::none())
            }

            _ => None,
        }
    }
}
