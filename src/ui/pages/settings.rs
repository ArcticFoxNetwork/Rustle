//! Settings page component
//!
//! All settings on one page with tab navigation (like markdown TOC)
//! Tab bar: continuous bottom line, active tab highlighted
//! Clicking tab scrolls to corresponding section

use iced::widget::{
    Space, button, column, container, pick_list, row, scrollable, svg, text, text_input, toggler,
};
use iced::{Alignment, Background, Border, Color, Element, Fill, Padding};

use crate::app::{Message, SettingsSection};
use crate::audio::get_audio_devices;
use crate::features::{Action, KeyBindings, Settings};
use crate::i18n::{Key, Locale};
use crate::ui::theme;

/// Settings page view with fixed header and all sections on one scrollable page
pub fn view(
    settings: &Settings,
    active_section: SettingsSection,
    locale: Locale,
    editing_keybinding: Option<Action>,
    is_logged_in: bool,
    user_info: Option<&crate::app::UserInfo>,
    cache_stats: Option<&crate::cache::CacheStats>,
) -> Element<'static, Message> {
    // Fixed header: title + tabs
    let header = column![
        text(locale.get(Key::SettingsTitle).to_string())
            .size(32)
            .style(|theme| text::Style {
                color: Some(theme::settings_title(theme))
            }),
        Space::new().height(24),
        tab_bar(active_section, locale),
    ]
    .width(Fill);

    let header_container = container(header)
        .width(Fill)
        .padding(Padding::new(40.0).right(60.0).bottom(20.0).left(60.0))
        .style(|theme| container::Style {
            background: Some(Background::Color(theme::background(theme))),
            ..Default::default()
        });

    // All sections on one page
    let all_sections = all_sections_content(
        settings,
        locale,
        editing_keybinding,
        is_logged_in,
        user_info,
        cache_stats,
    );

    let scrollable_content = scrollable(
        container(all_sections)
            .width(Fill)
            .padding(Padding::new(20.0).right(60.0).bottom(60.0).left(60.0)),
    )
    .width(Fill)
    .height(Fill)
    .id(iced::widget::Id::new("settings_scroll"))
    .on_scroll(|viewport| {
        let offset = viewport.absolute_offset();
        Message::SettingsScrolled(offset.y)
    });

    // Combine fixed header + scrollable content
    container(
        column![header_container, scrollable_content,]
            .width(Fill)
            .height(Fill),
    )
    .width(Fill)
    .height(Fill)
    .style(theme::main_content)
    .into()
}

/// Tab bar with continuous bottom line - active tab portion highlighted
fn tab_bar(active_section: SettingsSection, locale: Locale) -> Element<'static, Message> {
    let tabs = vec![
        (
            SettingsSection::Account,
            locale.get(Key::SettingsTabAccount),
        ),
        (
            SettingsSection::Playback,
            locale.get(Key::SettingsTabPlayback),
        ),
        (
            SettingsSection::Display,
            locale.get(Key::SettingsTabDisplay),
        ),
        (SettingsSection::System, locale.get(Key::SettingsTabSystem)),
        (
            SettingsSection::Network,
            locale.get(Key::SettingsTabNetwork),
        ),
        (
            SettingsSection::Storage,
            locale.get(Key::SettingsTabStorage),
        ),
        (
            SettingsSection::Shortcuts,
            locale.get(Key::SettingsTabShortcuts),
        ),
        (SettingsSection::About, locale.get(Key::SettingsTabAbout)),
    ];

    // Build tab items (button + underline stacked vertically)
    let tab_items: Vec<Element<'static, Message>> =
        tabs.iter()
            .map(|(section, label)| {
                let is_active = *section == active_section;

                let tab_button =
                    button(
                        container(text(label.to_string()).size(14).style(move |theme| {
                            text::Style {
                                color: Some(if is_active {
                                    theme::ACCENT_PINK
                                } else {
                                    theme::settings_inactive_tab(theme)
                                }),
                            }
                        }))
                        .width(Fill)
                        .center_x(Fill),
                    )
                    .style(move |theme, status| {
                        let hover_bg = match status {
                            button::Status::Hovered => {
                                Some(Background::Color(theme::hover_bg_alpha(theme, 0.05)))
                            }
                            _ => None,
                        };
                        button::Style {
                            background: hover_bg,
                            text_color: theme::text_primary(theme),
                            border: Border::default(),
                            ..Default::default()
                        }
                    })
                    .on_press(Message::ScrollToSection(*section))
                    .padding([12, 0])
                    .width(Fill);

                let underline = container(Space::new().height(2))
                    .width(Fill)
                    .style(move |theme| container::Style {
                        background: Some(Background::Color(if is_active {
                            theme::ACCENT_PINK
                        } else {
                            theme::settings_inactive_underline(theme)
                        })),
                        ..Default::default()
                    });

                container(column![tab_button, underline].spacing(0).width(Fill))
                    .width(90)
                    .into()
            })
            .collect();

    // All tabs in a row with horizontal scroll for narrow screens
    scrollable(row(tab_items).spacing(0))
        .direction(iced::widget::scrollable::Direction::Horizontal(
            iced::widget::scrollable::Scrollbar::new()
                .width(0)
                .scroller_width(0),
        ))
        .width(Fill)
        .into()
}

/// All settings sections on one page
fn all_sections_content(
    settings: &Settings,
    locale: Locale,
    editing_keybinding: Option<Action>,
    is_logged_in: bool,
    user_info: Option<&crate::app::UserInfo>,
    cache_stats: Option<&crate::cache::CacheStats>,
) -> Element<'static, Message> {
    column![
        // Account section
        section_header(locale.get(Key::SettingsAccountTitle)),
        Space::new().height(16),
        account_section(is_logged_in, user_info, locale),
        Space::new().height(40),
        // Playback section
        section_header(locale.get(Key::SettingsPlaybackTitle)),
        Space::new().height(16),
        playback_section(settings, locale),
        Space::new().height(40),
        // Display section
        section_header(locale.get(Key::SettingsDisplayTitle)),
        Space::new().height(16),
        display_section(settings, locale),
        Space::new().height(40),
        // System section
        section_header(locale.get(Key::SettingsSystemTitle)),
        Space::new().height(16),
        system_section(settings, locale),
        Space::new().height(40),
        // Network section
        section_header(locale.get(Key::SettingsNetworkTitle)),
        Space::new().height(16),
        network_section(settings, locale),
        Space::new().height(40),
        // Storage section
        section_header(locale.get(Key::SettingsStorageTitle)),
        Space::new().height(16),
        storage_section(settings, locale, cache_stats),
        Space::new().height(40),
        // Shortcuts section
        section_header(locale.get(Key::SettingsShortcutsTitle)),
        Space::new().height(16),
        shortcuts_section(&settings.keybindings, locale, editing_keybinding),
        Space::new().height(40),
        // About section
        section_header(locale.get(Key::SettingsAboutTitle)),
        Space::new().height(16),
        about_section(locale),
    ]
    .spacing(0)
    .width(Fill)
    .into()
}

fn account_section(
    is_logged_in: bool,
    user_info: Option<&crate::app::UserInfo>,
    locale: Locale,
) -> Element<'static, Message> {
    // Account section
    if is_logged_in {
        if let Some(info) = user_info {
            // Use pre-loaded avatar handle for instant rendering
            let avatar = if let Some(handle) = &info.avatar_handle {
                container(
                    iced::widget::image(handle.clone())
                        .width(Fill)
                        .height(Fill)
                        .content_fit(iced::ContentFit::Cover),
                )
                .width(48)
                .height(48)
                .clip(true)
                .style(|_theme| iced::widget::container::Style {
                    border: iced::Border {
                        radius: 24.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..Default::default()
                })
            } else {
                container(
                    svg(iced::widget::svg::Handle::from_memory(
                        crate::ui::icons::USER.as_bytes(),
                    ))
                    .width(24)
                    .height(24)
                    .style(|_theme, _status| iced::widget::svg::Style {
                        color: Some(theme::TEXT_SECONDARY),
                    }),
                )
                .width(48)
                .height(48)
                .center_x(48)
                .center_y(48)
                .style(|_theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(theme::BORDER_GRAY)),
                    border: iced::Border {
                        radius: 24.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
            };

            let vip_text = if info.vip_type > 0 {
                text("VIP").size(12).style(|_theme| text::Style {
                    color: Some(theme::ACCENT_PINK),
                })
            } else {
                text(locale.get(Key::FreeAccount))
                    .size(12)
                    .style(|theme| text::Style {
                        color: Some(theme::settings_desc(theme)),
                    })
            };

            column![
                setting_row(
                    locale.get(Key::SettingsAccountLoggedInAs),
                    None,
                    row![
                        avatar,
                        Space::new().width(12),
                        column![
                            text(info.nickname.clone())
                                .size(16)
                                .style(|theme| text::Style {
                                    color: Some(theme::text_primary(theme))
                                }),
                            vip_text,
                        ]
                    ]
                    .align_y(Alignment::Center)
                    .into()
                ),
                divider(),
                setting_row(
                    locale.get(Key::SettingsAccountLogout),
                    None,
                    button(text(locale.get(Key::SettingsAccountLogout).to_string()).size(14))
                        .style(theme::button_danger)
                        .padding([8, 16])
                        .on_press(Message::Logout)
                        .into()
                ),
            ]
            .spacing(0)
            .into()
        } else {
            // Logged in but no info yet
            column![
                setting_row(
                    locale.get(Key::SettingsAccountLoggedInAs),
                    None,
                    text("Loading...")
                        .size(14)
                        .style(|theme| text::Style {
                            color: Some(theme::text_primary(theme))
                        })
                        .into()
                ),
                divider(),
                setting_row(
                    locale.get(Key::SettingsAccountLogout),
                    None,
                    button(text(locale.get(Key::SettingsAccountLogout).to_string()).size(14))
                        .style(theme::button_danger)
                        .padding([8, 16])
                        .on_press(Message::Logout)
                        .into()
                ),
            ]
            .spacing(0)
            .into()
        }
    } else {
        column![setting_row(
            locale.get(Key::SettingsAccountNotLoggedIn),
            None,
            button(text(locale.get(Key::ClickToLogin).to_string()).size(14))
                .style(theme::primary_button)
                .padding([8, 16])
                .on_press(Message::ToggleLoginPopup)
                .into()
        ),]
        .spacing(0)
        .into()
    }
}

fn section_header(title: &str) -> Element<'static, Message> {
    text(title.to_string())
        .size(18)
        .style(|theme| text::Style {
            color: Some(theme::settings_section_title(theme)),
        })
        .into()
}

/// Setting row with label on left and control on right
fn setting_row<'a>(
    label: &str,
    description: Option<&str>,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    let label_text = label.to_string();
    let desc_text = description.map(|d| d.to_string());

    let label_section: Element<'a, Message> = if let Some(desc) = desc_text {
        column![
            text(label_text).size(15).style(|theme| text::Style {
                color: Some(theme::settings_label(theme))
            }),
            text(desc).size(12).style(|theme| text::Style {
                color: Some(theme::settings_desc(theme))
            }),
        ]
        .spacing(4)
        .into()
    } else {
        column![text(label_text).size(15).style(|theme| text::Style {
            color: Some(theme::settings_label(theme))
        }),]
        .into()
    };

    container(
        row![label_section, Space::new().width(Fill), control,]
            .align_y(Alignment::Center)
            .width(Fill),
    )
    .padding([16, 0])
    .into()
}

fn playback_section(settings: &Settings, locale: Locale) -> Element<'static, Message> {
    use crate::features::MusicQuality;

    // Build music quality options
    let quality_options: Vec<String> = MusicQuality::all()
        .iter()
        .map(|q| q.display_name().to_string())
        .collect();

    let current_quality = settings.playback.music_quality.display_name().to_string();

    column![
        setting_row(
            locale.get(Key::SettingsMusicQuality),
            Some(locale.get(Key::SettingsMusicQualityDesc)),
            styled_pick_list(quality_options, Some(current_quality), |value| {
                let quality = match value.as_str() {
                    "128kbps" => MusicQuality::Standard,
                    "192kbps" => MusicQuality::Higher,
                    "320kbps" => MusicQuality::High,
                    "SQ (无损)" => MusicQuality::Lossless,
                    "Hi-Res" => MusicQuality::HiRes,
                    _ => MusicQuality::High,
                };
                Message::UpdateMusicQuality(quality)
            },)
        ),
        divider(),
        setting_row(
            locale.get(Key::SettingsFadeInOut),
            Some(locale.get(Key::SettingsFadeInOutDesc)),
            toggler(settings.playback.fade_in_out)
                .on_toggle(Message::UpdateFadeInOut)
                .size(24)
                .into()
        ),
        divider(),
        setting_row(
            locale.get(Key::SettingsVolumeNormalization),
            Some(locale.get(Key::SettingsVolumeNormalizationDesc)),
            toggler(settings.playback.volume_normalization)
                .on_toggle(Message::UpdateVolumeNormalization)
                .size(24)
                .into()
        ),
        divider(),
        // Audio Engine entry - clickable row to navigate to audio engine page
        audio_engine_entry_row(locale),
    ]
    .spacing(0)
    .into()
}

/// Audio engine entry row - clickable to navigate to audio engine page
fn audio_engine_entry_row(locale: Locale) -> Element<'static, Message> {
    let content = row![
        // Title only
        text(locale.get(Key::AudioEngineTitle).to_string())
            .size(15)
            .style(|theme| text::Style {
                color: Some(theme::settings_label(theme))
            }),
        Space::new().width(Fill),
        // Chevron right icon
        svg(svg::Handle::from_memory(
            crate::ui::icons::CHEVRON_RIGHT.as_bytes()
        ))
        .width(20)
        .height(20)
        .style(|theme, _status| svg::Style {
            color: Some(theme::settings_desc(theme)),
        }),
    ]
    .align_y(Alignment::Center)
    .width(Fill);

    button(container(content).padding([16, 0]))
        .width(Fill)
        .padding(0)
        .style(|theme, status| {
            let bg = match status {
                button::Status::Hovered => Some(Background::Color(theme::hover_bg(theme))),
                button::Status::Pressed => Some(Background::Color(theme::hover_bg(theme))),
                _ => None,
            };
            button::Style {
                background: bg,
                border: Border::default(),
                text_color: Color::WHITE,
                ..Default::default()
            }
        })
        .on_press(Message::OpenAudioEngine)
        .into()
}

fn display_section(settings: &Settings, locale: Locale) -> Element<'static, Message> {
    use crate::features::CloseBehavior;

    let close_behavior_options = vec![
        locale.get(Key::SettingsCloseBehaviorAsk).to_string(),
        locale.get(Key::SettingsCloseBehaviorExit).to_string(),
        locale.get(Key::SettingsCloseBehaviorMinimize).to_string(),
    ];

    let current_close_behavior = match settings.close_behavior {
        CloseBehavior::Ask => locale.get(Key::SettingsCloseBehaviorAsk).to_string(),
        CloseBehavior::Exit => locale.get(Key::SettingsCloseBehaviorExit).to_string(),
        CloseBehavior::MinimizeToTray => locale.get(Key::SettingsCloseBehaviorMinimize).to_string(),
    };

    let ask_label = locale.get(Key::SettingsCloseBehaviorAsk).to_string();
    let exit_label = locale.get(Key::SettingsCloseBehaviorExit).to_string();

    column![
        setting_row(
            locale.get(Key::SettingsDarkMode),
            None,
            toggler(settings.display.dark_mode)
                .on_toggle(Message::UpdateDarkMode)
                .size(24)
                .into()
        ),
        divider(),
        setting_row(
            locale.get(Key::SettingsLanguage),
            None,
            styled_pick_list(
                vec!["简体中文".to_string(), "English".to_string()],
                Some(if settings.display.language == "zh" {
                    "简体中文".to_string()
                } else {
                    "English".to_string()
                }),
                |value| {
                    let lang = if value == "简体中文" { "zh" } else { "en" };
                    Message::UpdateAppLanguage(lang.to_string())
                },
            )
        ),
        divider(),
        setting_row(
            locale.get(Key::SettingsPowerSavingMode),
            Some(locale.get(Key::SettingsPowerSavingModeDesc)),
            toggler(settings.display.power_saving_mode)
                .on_toggle(Message::UpdatePowerSavingMode)
                .size(24)
                .into()
        ),
        divider(),
        setting_row(
            locale.get(Key::SettingsCloseBehavior),
            None,
            styled_pick_list(
                close_behavior_options,
                Some(current_close_behavior),
                move |value| {
                    let behavior = if value == ask_label {
                        CloseBehavior::Ask
                    } else if value == exit_label {
                        CloseBehavior::Exit
                    } else {
                        CloseBehavior::MinimizeToTray
                    };
                    Message::UpdateCloseBehavior(behavior)
                },
            )
        ),
    ]
    .spacing(0)
    .into()
}

fn system_section(settings: &Settings, locale: Locale) -> Element<'static, Message> {
    // Get real audio devices from PulseAudio/PipeWire
    let audio_devices = get_audio_devices();
    let default_device_label = locale.get(Key::SettingsDefaultDevice).to_string();

    // Build display names list (descriptions) and keep track of internal names
    let mut display_names: Vec<String> = vec![default_device_label.clone()];
    for device in &audio_devices {
        display_names.push(device.description.clone());
    }

    // Find current device's display name
    let current_display = if let Some(ref device_name) = settings.system.audio_output_device {
        audio_devices
            .iter()
            .find(|d| &d.name == device_name)
            .map(|d| d.description.clone())
            .unwrap_or_else(|| default_device_label.clone())
    } else {
        default_device_label.clone()
    };

    // Clone for closure
    let devices_for_closure = audio_devices.clone();
    let default_label = default_device_label.clone();

    column![setting_row(
        locale.get(Key::SettingsAudioDevice),
        None,
        styled_pick_list(display_names, Some(current_display), move |display_value| {
            // Convert display name back to internal name
            let device = if display_value == default_label {
                None
            } else {
                devices_for_closure
                    .iter()
                    .find(|d| d.description == display_value)
                    .map(|d| d.name.clone())
            };
            Message::UpdateAudioOutputDevice(device)
        },)
    ),]
    .spacing(0)
    .into()
}

fn network_section(settings: &Settings, locale: Locale) -> Element<'static, Message> {
    use crate::features::ProxyType;

    let proxy_types = vec![
        locale.get(Key::SettingsProxyNone).to_string(),
        "HTTP".to_string(),
        "HTTPS".to_string(),
        "SOCKS5".to_string(),
        locale.get(Key::SettingsProxySystem).to_string(),
    ];

    let current_proxy_type = match settings.network.proxy_type {
        ProxyType::None => locale.get(Key::SettingsProxyNone).to_string(),
        ProxyType::Http => "HTTP".to_string(),
        ProxyType::Https => "HTTPS".to_string(),
        ProxyType::Socks5 => "SOCKS5".to_string(),
        ProxyType::System => locale.get(Key::SettingsProxySystem).to_string(),
    };

    let proxy_none_label = locale.get(Key::SettingsProxyNone).to_string();

    let show_proxy_details = !matches!(
        settings.network.proxy_type,
        ProxyType::None | ProxyType::System
    );

    // Clone values for use in UI
    let proxy_host = settings.network.proxy_host.clone();
    let proxy_port = settings.network.proxy_port.to_string();
    let proxy_username = settings.network.proxy_username.clone().unwrap_or_default();
    let proxy_password = settings.network.proxy_password.clone().unwrap_or_default();

    let mut items: Vec<Element<'static, Message>> = vec![setting_row(
        locale.get(Key::SettingsProxyType),
        None,
        styled_pick_list(proxy_types, Some(current_proxy_type), move |value| {
            let proxy_type = if value == proxy_none_label {
                ProxyType::None
            } else if value == "HTTP" {
                ProxyType::Http
            } else if value == "HTTPS" {
                ProxyType::Https
            } else if value == "SOCKS5" {
                ProxyType::Socks5
            } else {
                ProxyType::System
            };
            Message::UpdateProxyType(proxy_type)
        }),
    )];

    if show_proxy_details {
        items.push(divider());
        items.push(setting_row_with_input(
            locale.get(Key::SettingsProxyHost),
            "127.0.0.1",
            &proxy_host,
            Message::UpdateProxyHost,
        ));
        items.push(divider());
        items.push(setting_row_with_input(
            locale.get(Key::SettingsProxyPort),
            "1080",
            &proxy_port,
            Message::UpdateProxyPort,
        ));
        items.push(divider());
        items.push(setting_row_with_input(
            locale.get(Key::SettingsProxyUsername),
            "",
            &proxy_username,
            Message::UpdateProxyUsername,
        ));
        items.push(divider());
        items.push(setting_row_with_input(
            locale.get(Key::SettingsProxyPassword),
            "",
            &proxy_password,
            Message::UpdateProxyPassword,
        ));
    }

    column(items).spacing(0).into()
}

/// Setting row with text input - handles lifetime issues by creating owned strings
fn setting_row_with_input<F>(
    label: &str,
    placeholder: &str,
    value: &str,
    on_input: F,
) -> Element<'static, Message>
where
    F: Fn(String) -> Message + 'static + Clone,
{
    let label_text = label.to_string();
    let placeholder_text = placeholder.to_string();
    let value_text = value.to_string();

    container(
        row![
            column![text(label_text).size(15).style(|theme| text::Style {
                color: Some(theme::settings_label(theme))
            }),],
            Space::new().width(Fill),
            text_input(&placeholder_text, &value_text)
                .on_input(on_input)
                .padding([8, 12])
                .width(200)
                .style(|theme, status| {
                    let border_color = match status {
                        text_input::Status::Focused { .. } => theme::ACCENT_PINK,
                        text_input::Status::Hovered => theme::settings_input_border_hover(theme),
                        _ => theme::settings_input_border(theme),
                    };
                    text_input::Style {
                        background: iced::Background::Color(theme::settings_input_bg(theme)),
                        border: Border {
                            color: border_color,
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        icon: theme::settings_desc(theme),
                        placeholder: theme::settings_desc(theme),
                        value: theme::settings_label(theme),
                        selection: theme::ACCENT_PINK,
                    }
                }),
        ]
        .align_y(Alignment::Center)
        .width(Fill),
    )
    .padding([16, 0])
    .into()
}

fn storage_section(
    settings: &Settings,
    locale: Locale,
    cache_stats: Option<&crate::cache::CacheStats>,
) -> Element<'static, Message> {
    // Get cache directory path
    let cache_dir = dirs::cache_dir()
        .map(|p| p.join("rustle"))
        .unwrap_or_else(|| std::path::PathBuf::from("~/.cache/rustle"));
    let cache_path_str = cache_dir.to_string_lossy().to_string();

    // Use cached stats if available, otherwise calculate on-demand
    let cache_size_str = if let Some(stats) = cache_stats {
        format_size_bytes(stats.total_bytes)
    } else {
        let cache_size = get_cache_size(&cache_dir);
        format_size_bytes(cache_size)
    };

    column![
        setting_row(
            locale.get(Key::SettingsCacheLocation),
            None,
            text(cache_path_str)
                .size(14)
                .style(|theme| text::Style {
                    color: Some(theme::settings_value(theme))
                })
                .into()
        ),
        divider(),
        setting_row(
            locale.get(Key::SettingsCacheSize),
            None,
            text(cache_size_str)
                .size(14)
                .style(|theme| text::Style {
                    color: Some(theme::settings_value(theme))
                })
                .into()
        ),
        divider(),
        setting_row(
            locale.get(Key::SettingsMaxCache),
            None,
            styled_pick_list(
                vec![
                    "512 MB".to_string(),
                    "1 GB".to_string(),
                    "2 GB".to_string(),
                    "5 GB".to_string()
                ],
                Some(format_cache_size(settings.storage.max_cache_mb)),
                |value| {
                    let size_mb = parse_cache_size(&value);
                    Message::UpdateMaxCacheMb(size_mb)
                },
            )
        ),
        divider(),
        setting_row(
            locale.get(Key::SettingsClearCache),
            Some(locale.get(Key::SettingsClearCacheDesc)),
            button(text(locale.get(Key::SettingsClearButton).to_string()).size(14))
                .style(theme::button_danger)
                .padding([8, 16])
                .on_press(Message::ClearCache)
                .into()
        ),
    ]
    .spacing(0)
    .into()
}

/// Get total size of cache directory in bytes
fn get_cache_size(path: &std::path::Path) -> u64 {
    if !path.exists() {
        return 0;
    }

    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Ok(meta) = path.metadata() {
                    total += meta.len();
                }
            } else if path.is_dir() {
                total += get_cache_size(&path);
            }
        }
    }
    total
}

/// Format bytes to human readable string
fn format_size_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn about_section(_locale: Locale) -> Element<'static, Message> {
    use once_cell::sync::Lazy;

    static ICON_DATA: &[u8] = include_bytes!("../../../assets/icons/icon_256.png");
    static ICON_HANDLE: Lazy<iced::widget::image::Handle> =
        Lazy::new(|| iced::widget::image::Handle::from_bytes(ICON_DATA));

    let icon = container(
        iced::widget::image(ICON_HANDLE.clone())
            .width(240)
            .height(240),
    )
    .style(|_theme| container::Style {
        border: Border {
            radius: 16.0.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .clip(true);

    // App name
    let app_name = text("Rustle").size(24).style(|theme| text::Style {
        color: Some(theme::text_primary(theme)),
    });

    // Version
    let version = text(format!("v{}", env!("CARGO_PKG_VERSION")))
        .size(14)
        .style(|theme| text::Style {
            color: Some(theme::settings_desc(theme)),
        });

    // Description
    let description = text("A modern music player built with Rust & Iced")
        .size(13)
        .style(|theme| text::Style {
            color: Some(theme::settings_desc(theme)),
        });

    // Copyright
    let copyright = text("2025 FXS").size(12).style(|theme| text::Style {
        color: Some(theme::settings_desc(theme)),
    });

    container(
        column![
            icon,
            Space::new().height(16),
            app_name,
            Space::new().height(4),
            version,
            Space::new().height(12),
            description,
            Space::new().height(8),
            copyright,
        ]
        .align_x(Alignment::Center),
    )
    .width(Fill)
    .center_x(Fill)
    .padding([40, 0])
    .into()
}

fn shortcuts_section(
    keybindings: &KeyBindings,
    locale: Locale,
    editing_keybinding: Option<Action>,
) -> Element<'static, Message> {
    // All actions split into two columns
    let left_actions = [
        (Action::PlayPause, Key::ActionPlayPause),
        (Action::NextTrack, Key::ActionNextTrack),
        (Action::PrevTrack, Key::ActionPrevTrack),
        (Action::VolumeUp, Key::ActionVolumeUp),
        (Action::VolumeDown, Key::ActionVolumeDown),
        (Action::VolumeMute, Key::ActionVolumeMute),
    ];

    let right_actions = [
        (Action::SeekForward, Key::ActionSeekForward),
        (Action::SeekBackward, Key::ActionSeekBackward),
        (Action::GoHome, Key::ActionGoHome),
        (Action::GoSearch, Key::ActionGoSearch),
        (Action::ToggleQueue, Key::ActionToggleQueue),
        (Action::ToggleFullscreen, Key::ActionToggleFullscreen),
    ];

    // Build left column
    let left_rows: Vec<Element<'static, Message>> = left_actions
        .iter()
        .map(|(action, key)| {
            let shortcut_text = keybindings.display_for_action(action);
            let is_editing = editing_keybinding == Some(*action);
            shortcut_row(*action, locale.get(*key), &shortcut_text, is_editing)
        })
        .collect();

    // Build right column
    let right_rows: Vec<Element<'static, Message>> = right_actions
        .iter()
        .map(|(action, key)| {
            let shortcut_text = keybindings.display_for_action(action);
            let is_editing = editing_keybinding == Some(*action);
            shortcut_row(*action, locale.get(*key), &shortcut_text, is_editing)
        })
        .collect();

    row![
        column(left_rows).spacing(4).width(Fill),
        Space::new().width(24),
        column(right_rows).spacing(4).width(Fill),
    ]
    .width(Fill)
    .into()
}

fn shortcut_row(
    action: Action,
    action_name: &str,
    shortcut: &str,
    is_editing: bool,
) -> Element<'static, Message> {
    let shortcut_display: Element<'static, Message> = if is_editing {
        container(
            text("Press key...".to_string())
                .size(13)
                .color(theme::ACCENT_PINK),
        )
        .padding([4, 12])
        .style(|theme| container::Style {
            background: Some(Background::Color(theme::shortcut_key_bg(theme))),
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: theme::ACCENT_PINK,
            },
            ..Default::default()
        })
        .into()
    } else {
        container(
            text(shortcut.to_string())
                .size(13)
                .style(|theme| text::Style {
                    color: Some(theme::settings_value(theme)),
                }),
        )
        .padding([4, 12])
        .style(|theme| container::Style {
            background: Some(Background::Color(theme::shortcut_bg(theme))),
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
    };

    let edit_button = button(shortcut_display)
        .style(|_, _| button::Style {
            background: None,
            text_color: Color::WHITE,
            border: Border::default(),
            ..Default::default()
        })
        .on_press(if is_editing {
            Message::CancelEditingKeybinding
        } else {
            Message::StartEditingKeybinding(action)
        });

    container(
        row![
            text(action_name.to_string())
                .size(14)
                .style(|theme| text::Style {
                    color: Some(theme::settings_label(theme))
                }),
            Space::new().width(Fill),
            edit_button,
        ]
        .align_y(Alignment::Center)
        .width(Fill),
    )
    .padding([8, 0])
    .into()
}

fn divider() -> Element<'static, Message> {
    container(Space::new().width(Fill).height(1))
        .style(|theme| container::Style {
            background: Some(Background::Color(theme::shortcut_bg(theme))),
            ..Default::default()
        })
        .width(Fill)
        .into()
}

/// Styled pick list (dropdown) with custom appearance
fn styled_pick_list<'a, T, F>(
    options: Vec<T>,
    selected: Option<T>,
    on_selected: F,
) -> Element<'a, Message>
where
    T: ToString + PartialEq + Clone + 'a,
    F: Fn(T) -> Message + 'a,
{
    pick_list(options, selected, on_selected)
        .style(theme::settings_pick_list)
        .menu_style(theme::settings_pick_list_menu)
        .padding([8, 12])
        .into()
}

fn format_cache_size(mb: u64) -> String {
    match mb {
        512 => "512 MB".to_string(),
        1024 => "1 GB".to_string(),
        2048 => "2 GB".to_string(),
        5120 => "5 GB".to_string(),
        10240 => "10 GB".to_string(),
        _ => format!("{} MB", mb),
    }
}

fn parse_cache_size(s: &str) -> u64 {
    match s {
        "512 MB" => 512,
        "1 GB" => 1024,
        "2 GB" => 2048,
        "5 GB" => 5120,
        "10 GB" => 10240,
        _ => 1024,
    }
}
