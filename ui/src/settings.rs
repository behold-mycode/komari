use std::fmt::Display;

use backend::{
    CaptureMode, FamiliarRarity, Familiars, InputMethod, IntoEnumIterator, Notifications,
    PanicMode, Settings as SettingsData, SwappableFamiliars, query_capture_handles, query_settings,
    select_capture_handle, update_settings, upsert_settings,
};
use dioxus::prelude::*;
use futures_util::StreamExt;
use tokio::task::spawn_blocking;

use crate::{
    AppState,
    button::{Button, ButtonKind},
    inputs::{Checkbox, MillisInput, TextInput},
    select::{EnumSelect, Select},
};

const INPUT_CLASS: &str = "h-6 w-full";

#[derive(Debug)]
enum SettingsUpdate {
    Set,
    Save,
}

#[component]
pub fn Settings() -> Element {
    let mut settings = use_context::<AppState>().settings;
    let settings_view = use_memo(move || settings().unwrap_or_default());

    // Handles async operations for settings-related
    let coroutine = use_coroutine(
        move |mut rx: UnboundedReceiver<SettingsUpdate>| async move {
            while let Some(message) = rx.next().await {
                match message {
                    SettingsUpdate::Set => {
                        update_settings(settings().expect("settings must be already set")).await;
                    }
                    SettingsUpdate::Save => {
                        let mut settings = settings().expect("settings must be already set");

                        spawn_blocking(move || {
                            upsert_settings(&mut settings).unwrap();
                        })
                        .await
                        .unwrap();
                    }
                }
            }
        },
    );
    let save_settings = use_callback(move |new_settings: SettingsData| {
        settings.set(Some(new_settings));
        coroutine.send(SettingsUpdate::Save);
        coroutine.send(SettingsUpdate::Set);
    });

    use_future(move || async move {
        if settings.peek().is_none() {
            settings.set(Some(spawn_blocking(query_settings).await.unwrap()));
        }
    });

    rsx! {
        div { class: "flex flex-col h-full overflow-y-auto scrollbar",
            SectionCapture { settings_view, save_settings }
            SectionInput { settings_view, save_settings }
            SectionFamiliars { settings_view, save_settings }
            SectionNotifications { settings_view, save_settings }
            SectionOthers { settings_view, save_settings }
        }
    }
}

#[component]
fn Section(name: &'static str, children: Element) -> Element {
    rsx! {
        div { class: "flex flex-col pr-4 pb-3",
            div { class: "flex items-center title-xs h-10", {name} }
            {children}
        }
    }
}

#[component]
fn SectionCapture(
    settings_view: ReadOnlySignal<SettingsData>,
    save_settings: EventHandler<SettingsData>,
) -> Element {
    let mut selected_handle_index = use_signal(|| None);
    let mut handle_names = use_resource(move || async move {
        let (names, selected) = query_capture_handles().await;
        selected_handle_index.set(selected);
        names
    });
    let handle_names_with_default = use_memo(move || {
        let default = vec!["Default".to_string()];
        let names = handle_names().unwrap_or_default();

        [default, names].concat()
    });

    rsx! {
        Section { name: "Capture",
            div { class: "grid grid-cols-2 gap-3",
                SettingsSelect {
                    label: "Handle",
                    options: handle_names_with_default(),
                    on_select: move |(index, _)| async move {
                        if index == 0 {
                            selected_handle_index.set(None);
                        } else {
                            selected_handle_index.set(Some(index - 1));
                            select_capture_handle(Some(index - 1)).await;
                        }
                    },
                    selected: selected_handle_index().unwrap_or_default(),
                }
                SettingsEnumSelect::<CaptureMode> {
                    label: "Mode",
                    on_select: move |capture_mode| {
                        save_settings(SettingsData {
                            capture_mode,
                            ..settings_view.peek().clone()
                        });
                    },
                    selected: settings_view().capture_mode,
                }
            }
            Button {
                text: "Refresh handles",
                kind: ButtonKind::Secondary,
                on_click: move |_| {
                    handle_names.restart();
                },
                class: "h-5 mt-2",
            }
        }
    }
}

#[component]
fn SectionInput(
    settings_view: ReadOnlySignal<SettingsData>,
    save_settings: EventHandler<SettingsData>,
) -> Element {
    let mut rpc_server_url = use_signal(move || settings_view().input_method_rpc_server_url);

    rsx! {
        Section { name: "Input",
            div { class: "grid grid-cols-3 gap-3",
                SettingsEnumSelect::<InputMethod> {
                    label: "Method",
                    on_select: move |input_method| async move {
                        save_settings(SettingsData {
                            input_method,
                            ..settings_view.peek().clone()
                        });
                    },
                    selected: settings_view().input_method,
                }
                TextInput {
                    label: "RPC server URL",
                    input_class: "h-6",
                    on_value: move |url| {
                        rpc_server_url.set(url);
                    },
                    value: rpc_server_url(),
                }
                Button {
                    text: "Update server url",
                    kind: ButtonKind::Primary,
                    on_click: move |_| {
                        save_settings(SettingsData {
                            input_method_rpc_server_url: rpc_server_url.peek().clone(),
                            ..settings_view.peek().clone()
                        });
                    },
                    class: "h-6 mt-5",
                }
            }
        }
    }
}

#[component]
fn SectionFamiliars(
    settings_view: ReadOnlySignal<SettingsData>,
    save_settings: EventHandler<SettingsData>,
) -> Element {
    let familiars_view = use_memo(move || settings_view().familiars);

    rsx! {
        Section { name: "Familiars",
            SettingsCheckbox {
                label: "Enable swapping",
                on_value: move |enable_familiars_swapping| {
                    save_settings(SettingsData {
                        familiars: Familiars {
                            enable_familiars_swapping,
                            ..familiars_view.peek().clone()
                        },
                        ..settings_view.peek().clone()
                    });
                },
                value: familiars_view().enable_familiars_swapping,
            }
            div { class: "grid grid-cols-2 gap-3",
                SettingsEnumSelect::<SwappableFamiliars> {
                    label: "Swappable slots",
                    disabled: !familiars_view().enable_familiars_swapping,
                    on_select: move |swappable_familiars| async move {
                        save_settings(SettingsData {
                            familiars: Familiars {
                                swappable_familiars,
                                ..familiars_view.peek().clone()
                            },
                            ..settings_view.peek().clone()
                        });
                    },
                    selected: familiars_view().swappable_familiars,
                }
                MillisInput {
                    label: "Swap check milliseconds",
                    input_class: "h-6",
                    disabled: !familiars_view().enable_familiars_swapping,
                    on_value: move |swap_check_millis| {
                        save_settings(SettingsData {
                            familiars: Familiars {
                                swap_check_millis,
                                ..familiars_view.peek().clone()
                            },
                            ..settings_view.peek().clone()
                        });
                    },
                    value: familiars_view().swap_check_millis,
                }

                SettingsCheckbox {
                    label: "Can swap rare familiars",
                    disabled: !familiars_view().enable_familiars_swapping,
                    on_value: move |allowed| {
                        let mut rarities = familiars_view.peek().swappable_rarities.clone();
                        if allowed {
                            rarities.insert(FamiliarRarity::Rare);
                        } else {
                            rarities.remove(&FamiliarRarity::Rare);
                        }
                        save_settings(SettingsData {
                            familiars: Familiars {
                                swappable_rarities: rarities,
                                ..familiars_view.peek().clone()
                            },
                            ..settings_view.peek().clone()
                        });
                    },
                    value: familiars_view().swappable_rarities.contains(&FamiliarRarity::Rare),
                }
                SettingsCheckbox {
                    label: "Can swap epic familiars",
                    disabled: !familiars_view().enable_familiars_swapping,
                    on_value: move |allowed| {
                        let mut rarities = familiars_view.peek().swappable_rarities.clone();
                        if allowed {
                            rarities.insert(FamiliarRarity::Epic);
                        } else {
                            rarities.remove(&FamiliarRarity::Epic);
                        }
                        save_settings(SettingsData {
                            familiars: Familiars {
                                swappable_rarities: rarities,
                                ..familiars_view.peek().clone()
                            },
                            ..settings_view.peek().clone()
                        });
                    },
                    value: familiars_view().swappable_rarities.contains(&FamiliarRarity::Epic),
                }
            }
        }
    }
}

#[component]
fn SectionNotifications(
    settings_view: ReadOnlySignal<SettingsData>,
    save_settings: EventHandler<SettingsData>,
) -> Element {
    let notifications_view = use_memo(move || settings_view().notifications);

    rsx! {
        Section { name: "Notifications",
            div { class: "grid grid-cols-3 gap-3",
                SettingsCheckbox {
                    label: "Rune spawns",
                    on_value: move |notify_on_rune_appear| {
                        save_settings(SettingsData {
                            notifications: Notifications {
                                notify_on_rune_appear,
                                ..notifications_view.peek().clone()
                            },
                            ..settings_view.peek().clone()
                        });
                    },
                    value: notifications_view().notify_on_rune_appear,
                }
                SettingsCheckbox {
                    label: "Elite boss spawns",
                    on_value: move |notify_on_elite_boss_appear| {
                        save_settings(SettingsData {
                            notifications: Notifications {
                                notify_on_elite_boss_appear,
                                ..notifications_view.peek().clone()
                            },
                            ..settings_view.peek().clone()
                        });
                    },
                    value: notifications_view().notify_on_elite_boss_appear,
                }
                SettingsCheckbox {
                    label: "Player dies",
                    on_value: move |notify_on_player_die| {
                        save_settings(SettingsData {
                            notifications: Notifications {
                                notify_on_player_die,
                                ..notifications_view.peek().clone()
                            },
                            ..settings_view.peek().clone()
                        });
                    },
                    value: notifications_view().notify_on_player_die,
                }
                SettingsCheckbox {
                    label: "Guildie appears",
                    on_value: move |notify_on_player_guildie_appear| {
                        save_settings(SettingsData {
                            notifications: Notifications {
                                notify_on_player_guildie_appear,
                                ..notifications_view.peek().clone()
                            },
                            ..settings_view.peek().clone()
                        });
                    },
                    value: notifications_view().notify_on_player_guildie_appear,
                }
                SettingsCheckbox {
                    label: "Stranger appears",
                    on_value: move |notify_on_player_stranger_appear| {
                        save_settings(SettingsData {
                            notifications: Notifications {
                                notify_on_player_stranger_appear,
                                ..notifications_view.peek().clone()
                            },
                            ..settings_view.peek().clone()
                        });
                    },
                    value: notifications_view().notify_on_player_stranger_appear,
                }
                SettingsCheckbox {
                    label: "Friend appears",
                    on_value: move |notify_on_player_friend_appear| {
                        save_settings(SettingsData {
                            notifications: Notifications {
                                notify_on_player_friend_appear,
                                ..notifications_view.peek().clone()
                            },
                            ..settings_view.peek().clone()
                        });
                    },
                    value: notifications_view().notify_on_player_friend_appear,
                }
                SettingsCheckbox {
                    label: "Detection fails or map changes",
                    on_value: move |notify_on_fail_or_change_map| {
                        save_settings(SettingsData {
                            notifications: Notifications {
                                notify_on_fail_or_change_map,
                                ..notifications_view.peek().clone()
                            },
                            ..settings_view.peek().clone()
                        });
                    },
                    value: notifications_view().notify_on_fail_or_change_map,
                }
            }
        }
    }
}

#[component]
fn SectionOthers(
    settings_view: ReadOnlySignal<SettingsData>,
    save_settings: EventHandler<SettingsData>,
) -> Element {
    rsx! {
        Section { name: "Others",
            div { class: "grid grid-cols-2 gap-3",
                SettingsCheckbox {
                    label: "Enable rune solving",
                    on_value: move |enable_rune_solving| {
                        save_settings(SettingsData {
                            enable_rune_solving,
                            ..settings_view.peek().clone()
                        });
                    },
                    value: settings_view().enable_rune_solving,
                }
                div {}
                SettingsCheckbox {
                    label: "Stop actions on fail or map changed",
                    on_value: move |stop_on_fail_or_change_map| {
                        save_settings(SettingsData {
                            stop_on_fail_or_change_map,
                            ..settings_view.peek().clone()
                        });
                    },
                    value: settings_view().stop_on_fail_or_change_map,
                }
                SettingsCheckbox {
                    label: "Change channel on elite boss spawns",
                    on_value: move |enable_change_channel_on_elite_boss_appear| {
                        save_settings(SettingsData {
                            enable_change_channel_on_elite_boss_appear,
                            ..settings_view.peek().clone()
                        });
                    },
                    value: settings_view().enable_change_channel_on_elite_boss_appear,
                }
                SettingsCheckbox {
                    label: "Enable panic mode",
                    on_value: move |enable_panic_mode| {
                        save_settings(SettingsData {
                            enable_panic_mode,
                            ..settings_view.peek().clone()
                        });
                    },
                    value: settings_view().enable_panic_mode,
                }
                SettingsEnumSelect::<PanicMode> {
                    label: "Mode",
                    disabled: !settings_view().enable_panic_mode,
                    on_select: move |panic_mode| async move {
                        save_settings(SettingsData {
                            panic_mode,
                            ..settings_view.peek().clone()
                        });
                    },
                    selected: settings_view().panic_mode,
                }
            }
        }
    }
}

#[component]
fn SettingsSelect<T: 'static + Clone + PartialEq + Display>(
    label: &'static str,
    options: Vec<T>,
    on_select: EventHandler<(usize, T)>,
    selected: usize,
) -> Element {
    rsx! {
        Select {
            label,
            select_class: INPUT_CLASS,
            options,
            on_select,
            selected,
        }
    }
}

#[component]
fn SettingsEnumSelect<T: 'static + Clone + PartialEq + Display + IntoEnumIterator>(
    label: &'static str,
    #[props(default = false)] disabled: bool,
    on_select: EventHandler<T>,
    selected: T,
) -> Element {
    rsx! {
        EnumSelect {
            label,
            select_class: INPUT_CLASS,
            disabled,
            on_select,
            selected,
        }
    }
}

#[component]
fn SettingsCheckbox(
    label: &'static str,
    #[props(default = false)] disabled: bool,
    on_value: EventHandler<bool>,
    value: bool,
) -> Element {
    rsx! {
        Checkbox {
            label,
            input_class: "w-6 h-6",
            disabled,
            on_value,
            value,
        }
    }
}
