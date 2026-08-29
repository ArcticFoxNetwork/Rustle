//! macOS menu-bar implementation.
//!
//! This remains a `tray-icon`/NSStatusItem backend, but all native objects are
//! owned and updated by the active AppKit/Winit UI thread. The deeper macOS UX
//! work (opt-in status item and dedicated template artwork) is intentionally a
//! separate task.

use super::{TrayCommand, TrayHandle, TrayState, TrayWindowCommand};
use crate::features::PlayMode;
use crate::i18n::{Key, Language, t};
use anyhow::{Context, anyhow};
use std::cell::RefCell;
use tokio::sync::mpsc;
use tray_icon::{
    TrayIcon, TrayIconBuilder,
    menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu},
};

const PLAY_PAUSE_ID: &str = "play_pause";
const PREV_TRACK_ID: &str = "prev_track";
const NEXT_TRACK_ID: &str = "next_track";
const TOGGLE_FAVORITE_ID: &str = "toggle_favorite";
const SEQUENTIAL_ID: &str = "sequential";
const LOOP_ALL_ID: &str = "loop_all";
const LOOP_ONE_ID: &str = "loop_one";
const SHUFFLE_ID: &str = "shuffle";
const TOGGLE_WINDOW_ID: &str = "toggle_window";
const QUIT_ID: &str = "quit";

thread_local! {
    static MACOS_TRAY: RefCell<Option<MacosTray>> = const { RefCell::new(None) };
}

pub fn start_macos_tray(
    language: Language,
    command_capacity: usize,
) -> anyhow::Result<(TrayHandle, mpsc::Receiver<TrayCommand>)> {
    let (command_tx, command_rx) = mpsc::channel(command_capacity);
    install_menu_handler(command_tx.clone());

    let state = TrayState::new(language);
    let (menu, items) = build_menu(&state)?;
    let icon = load_icon()?;
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(true)
        .with_tooltip(tooltip(&state))
        .with_icon(icon)
        .with_icon_as_template(true)
        .build()
        .map_err(|error| anyhow!("Failed to create macOS status item: {error}"))?;

    let owner = MacosTray { tray, items, state };
    MACOS_TRAY.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_some() {
            return Err(anyhow!("macOS status item is already initialized"));
        }
        *slot = Some(owner);
        Ok(())
    })?;

    Ok((TrayHandle { _private: () }, command_rx))
}

pub fn update_state(state: TrayState) -> anyhow::Result<()> {
    MACOS_TRAY.with(|slot| {
        let mut slot = slot.borrow_mut();
        let owner = slot
            .as_mut()
            .ok_or_else(|| anyhow!("macOS status item is not initialized"))?;
        owner.apply_state(state)
    })
}

pub fn set_language(language: Language) -> anyhow::Result<()> {
    MACOS_TRAY.with(|slot| {
        let mut slot = slot.borrow_mut();
        let owner = slot
            .as_mut()
            .ok_or_else(|| anyhow!("macOS status item is not initialized"))?;
        let mut state = owner.state.clone();
        state.language = language;
        owner.apply_state(state)
    })
}

pub fn shutdown() {
    MACOS_TRAY.with(|slot| {
        let _ = slot.borrow_mut().take();
    });
}

pub fn is_available() -> bool {
    MACOS_TRAY.with(|slot| slot.try_borrow().ok().is_some_and(|slot| slot.is_some()))
}

struct MacosTray {
    tray: TrayIcon,
    items: MenuItems,
    state: TrayState,
}

impl MacosTray {
    fn apply_state(&mut self, state: TrayState) -> anyhow::Result<()> {
        let language = state.language;
        self.items
            .now_playing
            .set_text(now_playing(&state, t(language, Key::TrayNotPlaying)));
        self.items.play_pause.set_text(t(
            language,
            if state.is_playing {
                Key::TrayPause
            } else {
                Key::TrayPlay
            },
        ));
        self.items.previous.set_text(t(language, Key::TrayPrevious));
        self.items.next.set_text(t(language, Key::TrayNext));
        self.items.favorite.set_text(t(
            language,
            if state.is_favorited && state.ncm_song_id.is_some() {
                Key::TrayUnfavorite
            } else {
                Key::TrayFavorite
            },
        ));
        self.items.favorite.set_enabled(state.ncm_song_id.is_some());
        self.items
            .play_mode_menu
            .set_text(t(language, Key::TrayPlayMode));
        self.items
            .sequential
            .set_text(t(language, Key::TraySequential));
        self.items.loop_all.set_text(t(language, Key::TrayLoopAll));
        self.items.loop_one.set_text(t(language, Key::TrayLoopOne));
        self.items.shuffle.set_text(t(language, Key::TrayShuffle));
        self.items
            .toggle_window
            .set_text(t(language, Key::TrayToggleWindow));
        self.items.quit.set_text(t(language, Key::TrayQuit));
        self.items
            .sequential
            .set_checked(state.play_mode == PlayMode::Sequential);
        self.items
            .loop_all
            .set_checked(state.play_mode == PlayMode::LoopAll);
        self.items
            .loop_one
            .set_checked(state.play_mode == PlayMode::LoopOne);
        self.items
            .shuffle
            .set_checked(state.play_mode == PlayMode::Shuffle);
        self.tray
            .set_tooltip(Some(tooltip(&state)))
            .context("Failed to update macOS status item tooltip")?;
        self.state = state;
        Ok(())
    }
}

struct MenuItems {
    now_playing: MenuItem,
    play_pause: MenuItem,
    previous: MenuItem,
    next: MenuItem,
    favorite: MenuItem,
    play_mode_menu: Submenu,
    sequential: CheckMenuItem,
    loop_all: CheckMenuItem,
    loop_one: CheckMenuItem,
    shuffle: CheckMenuItem,
    toggle_window: MenuItem,
    quit: MenuItem,
}

fn install_menu_handler(command_tx: mpsc::Sender<TrayCommand>) {
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let command = match event.id.0.as_str() {
            PLAY_PAUSE_ID => Some(TrayCommand::PlayPause),
            PREV_TRACK_ID => Some(TrayCommand::PrevTrack),
            NEXT_TRACK_ID => Some(TrayCommand::NextTrack),
            TOGGLE_FAVORITE_ID => Some(TrayCommand::ToggleFavorite),
            SEQUENTIAL_ID => Some(TrayCommand::SetPlayMode(PlayMode::Sequential)),
            LOOP_ALL_ID => Some(TrayCommand::SetPlayMode(PlayMode::LoopAll)),
            LOOP_ONE_ID => Some(TrayCommand::SetPlayMode(PlayMode::LoopOne)),
            SHUFFLE_ID => Some(TrayCommand::SetPlayMode(PlayMode::Shuffle)),
            TOGGLE_WINDOW_ID => Some(TrayCommand::Window(TrayWindowCommand::Toggle)),
            QUIT_ID => Some(TrayCommand::Quit),
            _ => None,
        };
        if let Some(command) = command {
            match command_tx.try_send(command) {
                Ok(()) => {}
                Err(mpsc::error::TrySendError::Full(_)) => {
                    tracing::warn!("macOS status item command channel is full; dropping input");
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    tracing::debug!("macOS status item command receiver is closed");
                }
            }
        }
    }));
}

fn build_menu(state: &TrayState) -> anyhow::Result<(Menu, MenuItems)> {
    let language = state.language;
    let menu = Menu::new();
    let now_playing = MenuItem::with_id(
        MenuId::new("now_playing"),
        now_playing(state, t(language, Key::TrayNotPlaying)),
        false,
        None,
    );
    let play_pause = MenuItem::with_id(
        MenuId::new(PLAY_PAUSE_ID),
        t(language, Key::TrayPlay),
        true,
        None,
    );
    let previous = MenuItem::with_id(
        MenuId::new(PREV_TRACK_ID),
        t(language, Key::TrayPrevious),
        true,
        None,
    );
    let next = MenuItem::with_id(
        MenuId::new(NEXT_TRACK_ID),
        t(language, Key::TrayNext),
        true,
        None,
    );
    let favorite = MenuItem::with_id(
        MenuId::new(TOGGLE_FAVORITE_ID),
        t(language, Key::TrayFavorite),
        false,
        None,
    );
    let play_mode_menu = Submenu::new(t(language, Key::TrayPlayMode), true);
    let sequential = CheckMenuItem::with_id(
        MenuId::new(SEQUENTIAL_ID),
        t(language, Key::TraySequential),
        true,
        true,
        None,
    );
    let loop_all = CheckMenuItem::with_id(
        MenuId::new(LOOP_ALL_ID),
        t(language, Key::TrayLoopAll),
        true,
        false,
        None,
    );
    let loop_one = CheckMenuItem::with_id(
        MenuId::new(LOOP_ONE_ID),
        t(language, Key::TrayLoopOne),
        true,
        false,
        None,
    );
    let shuffle = CheckMenuItem::with_id(
        MenuId::new(SHUFFLE_ID),
        t(language, Key::TrayShuffle),
        true,
        false,
        None,
    );
    let toggle_window = MenuItem::with_id(
        MenuId::new(TOGGLE_WINDOW_ID),
        t(language, Key::TrayToggleWindow),
        true,
        None,
    );
    let quit = MenuItem::with_id(MenuId::new(QUIT_ID), t(language, Key::TrayQuit), true, None);

    menu.append(&now_playing)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&play_pause)?;
    menu.append(&previous)?;
    menu.append(&next)?;
    menu.append(&favorite)?;
    menu.append(&PredefinedMenuItem::separator())?;
    play_mode_menu.append(&sequential)?;
    play_mode_menu.append(&loop_all)?;
    play_mode_menu.append(&loop_one)?;
    play_mode_menu.append(&shuffle)?;
    menu.append(&play_mode_menu)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&toggle_window)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&quit)?;

    Ok((
        menu,
        MenuItems {
            now_playing,
            play_pause,
            previous,
            next,
            favorite,
            play_mode_menu,
            sequential,
            loop_all,
            loop_one,
            shuffle,
            toggle_window,
            quit,
        },
    ))
}

fn load_icon() -> anyhow::Result<tray_icon::Icon> {
    static ICON_DATA: &[u8] = include_bytes!("../../../assets/icons/icon_256.png");
    let image = image::load_from_memory(ICON_DATA).context("Failed to load status item icon")?;
    let rgba = image
        .resize(36, 36, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    let (width, height) = rgba.dimensions();
    tray_icon::Icon::from_rgba(rgba.into_raw(), width, height)
        .map_err(|error| anyhow!("Failed to create status item icon: {error}"))
}

fn now_playing(state: &TrayState, fallback: &str) -> String {
    match (&state.title, &state.artist) {
        (Some(title), Some(artist)) if !artist.is_empty() => format!("♪ {title} — {artist}"),
        (Some(title), _) => format!("♪ {title}"),
        _ => fallback.to_string(),
    }
}

fn tooltip(state: &TrayState) -> String {
    format!(
        "Rustle — {}",
        now_playing(state, t(state.language, Key::TrayNotPlaying))
    )
}
