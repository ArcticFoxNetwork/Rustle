// src/app/update/window.rs
//! Window and exit dialog message handlers

use iced::Task;

use crate::app::message::Message;
use crate::app::state::{App, WindowVisibilityState};
use crate::features::CloseBehavior;
use crate::platform::window;

impl App {
    fn is_window_hidden(&self) -> bool {
        self.core.is_window_hidden()
    }

    fn is_window_visible_or_showing(&self) -> bool {
        matches!(
            self.core.window_visibility,
            WindowVisibilityState::Visible | WindowVisibilityState::Showing
        )
    }

    fn current_visible_mode(&self) -> iced::window::Mode {
        if self.is_window_visible_or_showing() {
            self.core.window_restore_mode
        } else {
            iced::window::Mode::Hidden
        }
    }

    fn begin_hide_window(&mut self) -> Task<Message> {
        if self.core.window_operation_pending
            || self.core.window_visibility == WindowVisibilityState::Hidden
            || self.core.window_visibility == WindowVisibilityState::Hiding
        {
            return Task::none();
        }

        tracing::info!(
            backend = if window::is_wayland_backend() {
                "wayland"
            } else {
                "x11"
            },
            "Hiding window to tray"
        );

        self.core.window_visibility = WindowVisibilityState::Hiding;
        self.core.window_operation_pending = true;

        window::set_window_mode(iced::window::Mode::Hidden)
            .chain(Task::done(Message::WindowOperationComplete))
    }

    fn begin_show_window(&mut self) -> Task<Message> {
        if self.core.window_operation_pending
            || self.core.window_visibility == WindowVisibilityState::Visible
            || self.core.window_visibility == WindowVisibilityState::Showing
        {
            return Task::none();
        }

        tracing::info!(
            backend = if window::is_wayland_backend() {
                "wayland"
            } else {
                "x11"
            },
            "Showing window"
        );

        self.core.window_visibility = WindowVisibilityState::Showing;
        self.core.window_operation_pending = true;

        if window::is_wayland_backend() {
            window::set_window_mode(self.core.window_restore_mode)
        } else {
            window::set_window_mode(self.core.window_restore_mode)
                .chain(Task::done(Message::WindowOperationComplete))
        }
    }

    fn toggle_window_task(&mut self) -> Task<Message> {
        if self.is_window_hidden() {
            self.begin_show_window()
        } else {
            self.begin_hide_window()
        }
    }

    fn begin_show_or_focus_window(&mut self) -> Task<Message> {
        if self.is_window_hidden() {
            self.begin_show_window()
        } else if self.core.window_focused {
            Task::none()
        } else {
            window::focus_window()
        }
    }

    /// Handle window-related messages
    pub fn handle_window(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::RequestClose => {
                match self.core.settings.close_behavior {
                    CloseBehavior::Ask => {
                        self.ui.dialogs.exit_open = true;
                        self.ui.dialogs.exit_animation.start();
                    }
                    CloseBehavior::Exit => {
                        return Some(iced::exit());
                    }
                    CloseBehavior::MinimizeToTray => {
                        return Some(self.begin_hide_window());
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
                self.ui.dialogs.exit_animation.stop();
                Some(self.begin_hide_window())
            }

            Message::CancelExit => {
                self.ui.dialogs.exit_open = false;
                self.ui.dialogs.exit_animation.stop();
                Some(Task::none())
            }

            Message::ExitDialogRememberChanged(checked) => {
                self.ui.dialogs.exit_remember = *checked;
                Some(Task::none())
            }

            Message::ToggleWindow => Some(self.toggle_window_task()),

            Message::ShowWindow => Some(self.begin_show_window()),

            Message::ShowOrFocusWindow => Some(self.begin_show_or_focus_window()),

            Message::WindowShown => {
                if self.core.window_visibility == WindowVisibilityState::Showing
                    && self.core.window_operation_pending
                    && window::is_wayland_backend()
                {
                    Some(Task::done(Message::WindowOperationComplete))
                } else {
                    Some(Task::none())
                }
            }

            Message::WindowFocused => {
                self.core.window_focused = true;
                self.sync_audio_analysis_state();
                Some(Task::none())
            }

            Message::WindowUnfocused => {
                self.core.window_focused = false;
                self.sync_audio_analysis_state();
                Some(Task::none())
            }

            Message::WindowOperationComplete => {
                self.core.window_operation_pending = false;
                self.core.window_visibility =
                    finalize_window_visibility(self.current_visible_mode());
                self.sync_audio_analysis_state();
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

fn finalize_window_visibility(window_mode: iced::window::Mode) -> WindowVisibilityState {
    if window_mode == iced::window::Mode::Hidden {
        WindowVisibilityState::Hidden
    } else {
        WindowVisibilityState::Visible
    }
}

#[cfg(test)]
mod tests {
    use super::finalize_window_visibility;
    use crate::app::state::WindowVisibilityState;

    #[test]
    fn finalize_window_operation_visibility() {
        assert_eq!(
            finalize_window_visibility(iced::window::Mode::Hidden),
            WindowVisibilityState::Hidden,
        );
        assert_eq!(
            finalize_window_visibility(iced::window::Mode::Windowed),
            WindowVisibilityState::Visible,
        );
        assert_eq!(
            finalize_window_visibility(iced::window::Mode::Fullscreen),
            WindowVisibilityState::Visible,
        );
    }
}
