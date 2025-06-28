use std::fmt::Display;

use backend::{
    ActionConfiguration, ActionConfigurationCondition, ActionKeyWith, Class, Configuration,
    IntoEnumIterator, KeyBinding, KeyBindingConfiguration, LinkKeyBinding, PotionMode,
    delete_config, query_configs, update_configuration, upsert_config,
};
use dioxus::prelude::*;
use futures_util::StreamExt;
use tokio::task::spawn_blocking;

use crate::{
    AppState,
    button::{Button, ButtonKind},
    icons::XIcon,
    inputs::{Checkbox, KeyBindingInput, MillisInput, NumberInputU32, PercentageInput},
    select::{EnumSelect, TextSelect},
};

#[derive(Debug)]
enum ConfigurationUpdate {
    Set,
    Save,
    Create(String),
    Delete,
    AddAction(ActionConfiguration),
    EditAction(ActionConfiguration, usize),
    DeleteAction(usize),
    ToggleAction(bool, usize),
}

#[derive(Clone, Copy, Debug)]
enum ActionConfigurationInputKind {
    Add(ActionConfiguration),
    Edit(ActionConfiguration, usize),
}

#[component]
pub fn Characters() -> Element {
    let mut config = use_context::<AppState>().config;
    let mut configs = use_resource(move || async move {
        spawn_blocking(|| query_configs().expect("failed to query configs"))
            .await
            .unwrap()
    });
    // Maps queried `configs` to names
    let config_names = use_memo(move || {
        configs()
            .unwrap_or_default()
            .into_iter()
            .map(|config| config.name)
            .collect()
    });
    // Maps currently selected `config` to the index in `configs`
    let config_index = use_memo(move || {
        configs().zip(config()).and_then(|(configs, config)| {
            configs
                .into_iter()
                .enumerate()
                .find(|(_, cfg)| config.id == cfg.id)
                .map(|(i, _)| i)
        })
    });
    // Default config if `config` is `None`
    let config_view = use_memo(move || config().unwrap_or_default());

    // Handles async operations for configuration-related
    let coroutine = use_coroutine(
        move |mut rx: UnboundedReceiver<ConfigurationUpdate>| async move {
            let mut save_config = async move |current_config: Configuration| {
                let mut save_config = current_config.clone();
                spawn_blocking(move || {
                    upsert_config(&mut save_config).expect("failed to upsert config actions");
                })
                .await
                .unwrap();
                config.set(Some(current_config));
                configs.restart();
            };

            while let Some(message) = rx.next().await {
                match message {
                    ConfigurationUpdate::Set => {
                        if let Some(config) = config() {
                            update_configuration(config).await;
                        }
                    }
                    ConfigurationUpdate::Save => {
                        let Some(mut config) = config() else {
                            continue;
                        };
                        debug_assert!(config.id.is_some(), "saving invalid config");

                        spawn_blocking(move || {
                            upsert_config(&mut config).unwrap();
                        })
                        .await
                        .unwrap();
                    }
                    ConfigurationUpdate::Create(name) => {
                        let mut new_config = Configuration {
                            name,
                            ..Configuration::default()
                        };
                        let mut save_config = new_config.clone();
                        let save_id = spawn_blocking(move || {
                            upsert_config(&mut save_config).unwrap();
                            save_config
                                .id
                                .expect("config id must be valid after creation")
                        })
                        .await
                        .unwrap();

                        new_config.id = Some(save_id);
                        config.set(Some(new_config));
                        configs.restart();
                    }
                    ConfigurationUpdate::Delete => {
                        if let Some(config) = config.take() {
                            spawn_blocking(move || {
                                delete_config(&config).expect("failed to delete config");
                            })
                            .await
                            .unwrap();
                            configs.restart();
                        }
                    }
                    ConfigurationUpdate::AddAction(action) => {
                        let Some(mut config) = config() else {
                            continue;
                        };

                        config.actions.push(action);
                        save_config(config).await;
                    }
                    ConfigurationUpdate::EditAction(action, index) => {
                        let Some(mut config) = config() else {
                            continue;
                        };

                        *config.actions.get_mut(index).unwrap() = action;
                        save_config(config).await;
                    }
                    ConfigurationUpdate::DeleteAction(index) => {
                        let Some(mut config) = config() else {
                            continue;
                        };

                        config.actions.remove(index);
                        save_config(config).await;
                    }
                    ConfigurationUpdate::ToggleAction(enabled, index) => {
                        let Some(mut config) = config() else {
                            continue;
                        };

                        let config_mut = config.actions.get_mut(index).unwrap();
                        config_mut.enabled = enabled;
                        save_config(config).await;
                    }
                }
            }
        },
    );
    let save_config = use_callback(move |new_config: Configuration| {
        config.set(Some(new_config));
        coroutine.send(ConfigurationUpdate::Save);
        coroutine.send(ConfigurationUpdate::Set);
    });
    let action_input_kind = use_signal(|| None);

    // Sets a configuration if there is not one
    use_effect(move || {
        if let Some(configs) = configs()
            && config.peek().is_none()
        {
            config.set(configs.into_iter().next());
            coroutine.send(ConfigurationUpdate::Set);
        }
    });

    rsx! {
        div { class: "flex flex-col pb-15 h-full overflow-y-auto scrollbar",
            SectionKeyBindings { config_view, save_config }
            SectionBuffs { config_view, save_config }
            SectionFixedActions { action_input_kind, config_view, save_config }
            SectionOthers { config_view, save_config }
        }
        PopupActionConfigurationInput { action_input_kind, actions: config_view().actions }
        div { class: "flex items-center w-full h-10 bg-gray-950 absolute bottom-0 pr-2",
            TextSelect {
                class: "flex-grow",
                options: config_names(),
                disabled: false,
                on_create: move |name| {
                    coroutine.send(ConfigurationUpdate::Create(name));
                    coroutine.send(ConfigurationUpdate::Set);
                },
                on_delete: move |_| {
                    coroutine.send(ConfigurationUpdate::Delete);
                },
                on_select: move |(index, _)| {
                    let selected = configs.peek().as_ref().unwrap().get(index).cloned().unwrap();
                    config.set(Some(selected));
                    coroutine.send(ConfigurationUpdate::Set);
                },
                selected: config_index(),
            }
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
fn SectionKeyBindings(
    config_view: Memo<Configuration>,
    save_config: Callback<Configuration>,
) -> Element {
    rsx! {
        Section { name: "Key bindings",
            div { class: "grid grid-cols-2 gap-4",
                KeyBindingConfigurationInput {
                    label: "Rope lift",
                    optional: true,
                    on_value: move |ropelift_key| {
                        save_config(Configuration {
                            ropelift_key,
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().ropelift_key,
                }
                KeyBindingConfigurationInput {
                    label: "Teleport",
                    optional: true,
                    on_value: move |teleport_key| {
                        save_config(Configuration {
                            teleport_key,
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().teleport_key,
                }
                KeyBindingConfigurationInput {
                    label: "Jump",
                    on_value: move |key_config: Option<KeyBindingConfiguration>| {
                        save_config(Configuration {
                            jump_key: key_config.expect("not optional"),
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().jump_key,
                }
                KeyBindingConfigurationInput {
                    label: "Up jump",
                    optional: true,
                    on_value: move |up_jump_key| {
                        save_config(Configuration {
                            up_jump_key,
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().up_jump_key,
                }
                KeyBindingConfigurationInput {
                    label: "Interact",
                    on_value: move |key_config: Option<KeyBindingConfiguration>| {
                        save_config(Configuration {
                            interact_key: key_config.expect("not optional"),
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().interact_key,
                }
                KeyBindingConfigurationInput {
                    label: "Cash shop",
                    on_value: move |key_config: Option<KeyBindingConfiguration>| {
                        save_config(Configuration {
                            cash_shop_key: key_config.expect("not optional"),
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().cash_shop_key,
                }
                KeyBindingConfigurationInput {
                    label: "Maple guide",
                    on_value: move |key_config: Option<KeyBindingConfiguration>| {
                        save_config(Configuration {
                            maple_guide_key: key_config.expect("not optional"),
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().maple_guide_key,
                }
                KeyBindingConfigurationInput {
                    label: "Change channel",
                    on_value: move |key_config: Option<KeyBindingConfiguration>| {
                        save_config(Configuration {
                            change_channel_key: key_config.expect("not optional"),
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().change_channel_key,
                }
                KeyBindingConfigurationInput {
                    label: "Feed pet",
                    on_value: move |key_config: Option<KeyBindingConfiguration>| {
                        save_config(Configuration {
                            feed_pet_key: key_config.expect("not optional"),
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().feed_pet_key,
                }
                KeyBindingConfigurationInput {
                    label: "Potion",
                    on_value: move |key_config: Option<KeyBindingConfiguration>| {
                        save_config(Configuration {
                            potion_key: key_config.expect("not optional"),
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().potion_key,
                }
                div { class: "col-span-full grid-cols-3 grid gap-2 justify-items-stretch",
                    KeyBindingConfigurationInput {
                        label: "Familiar menu",
                        on_value: move |key_config: Option<KeyBindingConfiguration>| {
                            save_config(Configuration {
                                familiar_menu_key: key_config.expect("not optional"),
                                ..config_view.peek().clone()
                            });
                        },
                        value: config_view().familiar_menu_key,
                    }
                    KeyBindingConfigurationInput {
                        label: "Familiar skill",
                        on_value: move |key_config: Option<KeyBindingConfiguration>| {
                            save_config(Configuration {
                                familiar_buff_key: key_config.expect("not optional"),
                                ..config_view.peek().clone()
                            });
                        },
                        value: config_view().familiar_buff_key,
                    }
                    KeyBindingConfigurationInput {
                        label: "Familiar essence",
                        on_value: move |key_config: Option<KeyBindingConfiguration>| {
                            save_config(Configuration {
                                familiar_essence_key: key_config.expect("not optional"),
                                ..config_view.peek().clone()
                            });
                        },
                        value: config_view().familiar_essence_key,
                    }
                }
            }
        }
    }
}

#[component]
fn SectionBuffs(config_view: Memo<Configuration>, save_config: Callback<Configuration>) -> Element {
    #[component]
    fn Buff(
        label: &'static str,
        on_value: EventHandler<KeyBindingConfiguration>,
        value: KeyBindingConfiguration,
    ) -> Element {
        rsx! {
            div { class: "grid grid-cols-[140px_auto] gap-2",
                KeyBindingConfigurationInput {
                    label,
                    on_value: move |config: Option<KeyBindingConfiguration>| {
                        on_value(config.expect("not optional"));
                    },
                    value: Some(value),
                }
                Checkbox {
                    label: "Enabled",
                    on_value: move |enabled| {
                        on_value(KeyBindingConfiguration {
                            enabled,
                            ..value
                        });
                    },
                    value: value.enabled,
                    input_class: "w-6",
                }
            }
        }
    }

    rsx! {
        Section { name: "Buffs",
            div { class: "grid grid-cols-2 gap-4",
                Buff {
                    label: "Sayram's Elixir",
                    on_value: move |sayram_elixir_key| {
                        save_config(Configuration {
                            sayram_elixir_key,
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().sayram_elixir_key,
                }
                Buff {
                    label: "Aurelia's Elixir",
                    on_value: move |aurelia_elixir_key| {
                        save_config(Configuration {
                            aurelia_elixir_key,
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().aurelia_elixir_key,
                }
                Buff {
                    label: "3x EXP Coupon",
                    on_value: move |exp_x3_key| {
                        save_config(Configuration {
                            exp_x3_key,
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().exp_x3_key,
                }
                Buff {
                    label: "50% Bonus EXP Coupon",
                    on_value: move |bonus_exp_key| {
                        save_config(Configuration {
                            bonus_exp_key,
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().bonus_exp_key,
                }
                Buff {
                    label: "Legion's Wealth",
                    on_value: move |legion_wealth_key| {
                        save_config(Configuration {
                            legion_wealth_key,
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().legion_wealth_key,
                }
                Buff {
                    label: "Legion's Luck",
                    on_value: move |legion_luck_key| {
                        save_config(Configuration {
                            legion_luck_key,
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().legion_luck_key,
                }
                Buff {
                    label: "Wealth Acquisition Potion",
                    on_value: move |wealth_acquisition_potion_key| {
                        save_config(Configuration {
                            wealth_acquisition_potion_key,
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().wealth_acquisition_potion_key,
                }
                Buff {
                    label: "EXP Accumulation Potion",
                    on_value: move |exp_accumulation_potion_key| {
                        save_config(Configuration {
                            exp_accumulation_potion_key,
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().exp_accumulation_potion_key,
                }
                Buff {
                    label: "Extreme Red Potion",
                    on_value: move |extreme_red_potion_key| {
                        save_config(Configuration {
                            extreme_red_potion_key,
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().extreme_red_potion_key,
                }
                Buff {
                    label: "Extreme Blue Potion",
                    on_value: move |extreme_blue_potion_key| {
                        save_config(Configuration {
                            extreme_blue_potion_key,
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().extreme_blue_potion_key,
                }
                Buff {
                    label: "Extreme Green Potion",
                    on_value: move |extreme_green_potion_key| {
                        save_config(Configuration {
                            extreme_green_potion_key,
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().extreme_green_potion_key,
                }
                Buff {
                    label: "Extreme Gold Potion",
                    on_value: move |extreme_gold_potion_key| {
                        save_config(Configuration {
                            extreme_gold_potion_key,
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().extreme_gold_potion_key,
                }
            }
        }
    }
}

#[component]
fn SectionFixedActions(
    action_input_kind: Signal<Option<ActionConfigurationInputKind>>,
    config_view: Memo<Configuration>,
    save_config: Callback<Configuration>,
) -> Element {
    let coroutine = use_coroutine_handle::<ConfigurationUpdate>();

    rsx! {
        Section { name: "Fixed actions",
            ActionConfigurationList {
                on_add_click: move |_| {
                    action_input_kind
                        .set(
                            Some(ActionConfigurationInputKind::Add(ActionConfiguration::default())),
                        );
                },
                on_item_click: move |(action, index)| {
                    action_input_kind.set(Some(ActionConfigurationInputKind::Edit(action, index)));
                },
                on_item_delete: move |index| {
                    coroutine.send(ConfigurationUpdate::DeleteAction(index));
                    coroutine.send(ConfigurationUpdate::Set);
                },
                on_item_toggle: move |(enabled, index)| {
                    coroutine.send(ConfigurationUpdate::ToggleAction(enabled, index));
                    coroutine.send(ConfigurationUpdate::Set);
                },
                actions: config_view().actions,
            }
        }
    }
}

#[component]
fn SectionOthers(
    config_view: Memo<Configuration>,
    save_config: Callback<Configuration>,
) -> Element {
    rsx! {
        Section { name: "Others",
            div { class: "grid grid-cols-2 gap-4",
                CharactersMillisInput {
                    label: "Feed pet every milliseconds",
                    on_value: move |feed_pet_millis| {
                        save_config(Configuration {
                            feed_pet_millis,
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().feed_pet_millis,
                }
                div {} // Spacer

                CharactersSelect::<PotionMode> {
                    label: "Use potion mode",
                    on_select: move |potion_mode| {
                        save_config(Configuration {
                            potion_mode,
                            ..config_view.peek().clone()
                        });
                    },
                    selected: config_view().potion_mode,
                }
                match config_view().potion_mode {
                    PotionMode::EveryMillis(millis) => rsx! {
                        CharactersMillisInput {
                            label: "Use potion every milliseconds",
                            on_value: move |millis| {
                                save_config(Configuration {
                                    potion_mode: PotionMode::EveryMillis(millis),
                                    ..config_view.peek().clone()
                                });
                            },
                            value: millis,
                        }
                    },
                    PotionMode::Percentage(percent) => rsx! {
                        CharactersPercentageInput {
                            label: "Use potion health below percentage",
                            on_value: move |percent| {
                                save_config(Configuration {
                                    potion_mode: PotionMode::Percentage(percent),
                                    ..config_view.peek().clone()
                                });
                            },
                            value: percent,
                        }
                    },
                }

                CharactersSelect::<Class> {
                    label: "Link key timing class",
                    on_select: move |class| {
                        save_config(Configuration {
                            class,
                            ..config_view.peek().clone()
                        });
                    },
                    selected: config_view().class,
                }
                CharactersCheckbox {
                    label: "Disable walking",
                    on_value: move |disable_adjusting| {
                        save_config(Configuration {
                            disable_adjusting,
                            ..config_view.peek().clone()
                        });
                    },
                    value: config_view().disable_adjusting,
                }
            }
        }
    }
}

#[component]
fn KeyBindingConfigurationInput(
    label: &'static str,
    #[props(default = false)] optional: bool,
    on_value: EventHandler<Option<KeyBindingConfiguration>>,
    value: Option<KeyBindingConfiguration>,
) -> Element {
    let label = if optional {
        format!("{label} (optional)")
    } else {
        label.to_string()
    };

    rsx! {
        KeyBindingInput {
            label,
            optional,
            on_value: move |new_value: Option<KeyBinding>| {
                let new_value = new_value
                    .map(|key| {
                        let mut config = value.unwrap_or_default();
                        config.key = key;
                        config
                    });
                on_value(new_value);
            },
            value: value.map(|config| config.key),
        }
    }
}

#[component]
fn CharactersCheckbox(
    label: &'static str,
    #[props(default = String::default())] label_class: String,
    on_value: EventHandler<bool>,
    value: bool,
) -> Element {
    rsx! {
        Checkbox {
            label,
            label_class,
            input_class: "w-6",
            on_value,
            value,
        }
    }
}

#[component]
fn CharactersSelect<T: 'static + Clone + PartialEq + Display + IntoEnumIterator>(
    label: &'static str,
    #[props(default = false)] disabled: bool,
    on_select: EventHandler<T>,
    selected: T,
) -> Element {
    rsx! {
        EnumSelect {
            label,
            disabled,
            on_select,
            selected,
        }
    }
}

#[component]
fn CharactersPercentageInput(
    label: &'static str,
    on_value: EventHandler<f32>,
    value: f32,
) -> Element {
    rsx! {
        PercentageInput { label, on_value, value }
    }
}

#[component]
fn CharactersMillisInput(
    label: &'static str,
    #[props(default = false)] disabled: bool,
    on_value: EventHandler<u64>,
    value: u64,
) -> Element {
    rsx! {
        MillisInput {
            label,
            disabled,
            on_value,
            value,
        }
    }
}

#[component]
fn PopupActionConfigurationInput(
    action_input_kind: Signal<Option<ActionConfigurationInputKind>>,
    actions: ReadOnlySignal<Vec<ActionConfiguration>>,
) -> Element {
    #[derive(PartialEq, Clone, Debug)]
    struct State {
        action: ActionConfiguration,
        modifying: bool,
        section_text: String,
        can_create_linked_action: bool,
    }

    let state = use_memo(move || {
        action_input_kind().map(|kind| {
            let (action, index) = match kind {
                ActionConfigurationInputKind::Add(action) => (action, None),
                ActionConfigurationInputKind::Edit(action, index) => (action, Some(index)),
            };
            let modifying = matches!(kind, ActionConfigurationInputKind::Edit(_, _));
            let can_create_linked_action = match action.condition {
                ActionConfigurationCondition::EveryMillis(_) => {
                    !actions().is_empty() && index != Some(0)
                }
                ActionConfigurationCondition::Linked => false,
            };
            let section_text = if modifying {
                "Modify a fixed action".to_string()
            } else {
                "Add a new fixed action".to_string()
            };

            State {
                action,
                modifying,
                section_text,
                can_create_linked_action,
            }
        })
    });
    let coroutine = use_coroutine_handle::<ConfigurationUpdate>();

    rsx! {
        if let Some(State { action, modifying, section_text, can_create_linked_action }) = state() {
            div { class: "p-8 w-full h-full absolute inset-0 z-1 bg-gray-950/80",
                div { class: "bg-gray-900 h-full px-2",
                    div { class: "flex flex-col gap-2 relative h-full",
                        div { class: "flex flex-none items-center title-xs h-10", {section_text} }
                        ActionConfigurationInput {
                            modifying,
                            can_create_linked_action,
                            on_cancel: move |_| {
                                action_input_kind.set(None);
                            },
                            on_value: move |action| {
                                let update = match action_input_kind
                                    .take()
                                    .expect("input kind must already be set")
                                {
                                    ActionConfigurationInputKind::Add(_) => {
                                        ConfigurationUpdate::AddAction(action)
                                    }
                                    ActionConfigurationInputKind::Edit(_, index) => {
                                        ConfigurationUpdate::EditAction(action, index)
                                    }
                                };
                                coroutine.send(update);
                                coroutine.send(ConfigurationUpdate::Set);
                            },
                            value: action,
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ActionConfigurationInput(
    modifying: bool,
    can_create_linked_action: bool,
    on_cancel: EventHandler,
    on_value: EventHandler<ActionConfiguration>,
    value: ActionConfiguration,
) -> Element {
    let mut action = use_signal(|| value);
    let millis = use_memo(move || match action().condition {
        ActionConfigurationCondition::EveryMillis(millis) => Some(millis),
        ActionConfigurationCondition::Linked => None,
    });

    use_effect(use_reactive!(|value| { action.set(value) }));

    rsx! {
        div { class: "grid grid-cols-3 gap-3 pb-10 overflow-y-auto scrollbar",
            // Key, count and link key
            KeyBindingInput {
                label: "Key",
                input_class: "border border-gray-600",
                on_value: move |key: Option<KeyBinding>| {
                    let mut action = action.write();
                    action.key = key.expect("not optional");
                },
                value: Some(action().key),
            }
            NumberInputU32 {
                label: "Use count",
                on_value: move |count| {
                    let mut action = action.write();
                    action.count = count;
                },
                minimum_value: 1,
                value: action().count,
            }
            if can_create_linked_action {
                CharactersCheckbox {
                    label: "Linked action",
                    on_value: move |is_linked: bool| {
                        let mut action = action.write();
                        action.condition = if is_linked {
                            ActionConfigurationCondition::Linked
                        } else {
                            value.condition
                        };
                    },
                    value: matches!(action().condition, ActionConfigurationCondition::Linked),
                }
            } else {
                div {} // Spacer
            }
            KeyBindingInput {
                label: "Link key",
                input_class: "border border-gray-600",
                disabled: action().link_key.is_none(),
                on_value: move |key: Option<KeyBinding>| {
                    let mut action = action.write();
                    action.link_key = action
                        .link_key
                        .map(|link_key| link_key.with_key(key.expect("not optional")));
                },
                value: action().link_key.unwrap_or_default().key(),
            }
            CharactersSelect::<LinkKeyBinding> {
                label: "Link key type",
                disabled: action().link_key.is_none(),
                on_select: move |link_key: LinkKeyBinding| {
                    let mut action = action.write();
                    action.link_key = Some(
                        link_key.with_key(action.link_key.expect("has link key if selectable").key()),
                    );
                },
                selected: action().link_key.unwrap_or_default(),
            }
            CharactersCheckbox {
                label: "Has link key",
                on_value: move |has_link_key: bool| {
                    let mut action = action.write();
                    action.link_key = has_link_key.then_some(LinkKeyBinding::default());
                },
                value: action().link_key.is_some(),
            }

            // Use with
            CharactersSelect::<ActionKeyWith> {
                label: "Use with",
                on_select: move |with| {
                    let mut action = action.write();
                    action.with = with;
                },
                selected: action().with,
            }
            CharactersMillisInput {
                label: "Use every",
                disabled: millis().is_none(),
                on_value: move |new_millis| {
                    if millis.peek().is_some() {
                        let mut action = action.write();
                        action.condition = ActionConfigurationCondition::EveryMillis(new_millis);
                    }
                },
                value: millis().unwrap_or_default(),
            }
            div {} // Spacer

            // Wait before use
            CharactersMillisInput {
                label: "Wait before",
                on_value: move |millis| {
                    let mut action = action.write();
                    action.wait_before_millis = millis;
                },
                value: action().wait_before_millis,
            }
            CharactersMillisInput {
                label: "Wait random range",
                on_value: move |millis| {
                    let mut action = action.write();
                    action.wait_before_millis_random_range = millis;
                },
                value: action().wait_before_millis_random_range,
            }
            div {} // Spacer

            // Wait after use
            CharactersMillisInput {
                label: "Wait after",
                on_value: move |millis| {
                    let mut action = action.write();
                    action.wait_after_millis = millis;
                },
                value: action().wait_after_millis,
            }
            CharactersMillisInput {
                label: "Wait random range",
                on_value: move |millis| {
                    let mut action = action.write();
                    action.wait_after_millis_random_range = millis;
                },
                value: action().wait_after_millis_random_range,
            }
        }
        div { class: "flex w-full gap-3 absolute bottom-0 py-2 bg-gray-900",
            Button {
                class: "flex-grow border border-gray-600",
                text: if modifying { "Save" } else { "Add" },
                kind: ButtonKind::Primary,
                on_click: move |_| {
                    on_value(*action.peek());
                },
            }
            Button {
                class: "flex-grow border border-gray-600",
                text: "Cancel",
                kind: ButtonKind::Danger,
                on_click: move |_| {
                    on_cancel(());
                },
            }
        }
    }
}

#[component]
fn ActionConfigurationList(
    on_add_click: EventHandler,
    on_item_click: EventHandler<(ActionConfiguration, usize)>,
    on_item_delete: EventHandler<usize>,
    on_item_toggle: EventHandler<(bool, usize)>,
    actions: Vec<ActionConfiguration>,
) -> Element {
    #[component]
    fn Icons(condition: ActionConfigurationCondition, on_item_delete: EventHandler) -> Element {
        const ICON_CONTAINER_CLASS: &str = "w-4 h-6 flex justify-center items-center";
        const ICON_CLASS: &str = "w-[11px] h-[11px] fill-current";

        let container_margin = if matches!(condition, ActionConfigurationCondition::Linked) {
            ""
        } else {
            "mt-2"
        };
        rsx! {
            div { class: "absolute invisible group-hover:visible top-0 right-1 flex {container_margin}",
                div {
                    class: ICON_CONTAINER_CLASS,
                    onclick: move |e| {
                        e.stop_propagation();
                        on_item_delete(());
                    },
                    XIcon { class: "{ICON_CLASS} text-red-500" }
                }
            }
        }
    }

    rsx! {
        div { class: "flex flex-col",
            for (index , action) in actions.into_iter().enumerate() {
                div { class: "flex items-end",
                    div {
                        class: "relative group flex-grow",
                        onclick: move |e| {
                            e.stop_propagation();
                            on_item_click((action, index));
                        },
                        ActionConfigurationItem { action }
                        Icons {
                            condition: action.condition,
                            on_item_delete: move |_| {
                                on_item_delete(index);
                            },
                        }
                    }
                    div { class: "w-8 flex flex-col items-end",
                        if !matches!(action.condition, ActionConfigurationCondition::Linked) {
                            CharactersCheckbox {
                                label: "",
                                label_class: "collapse",
                                on_value: move |enabled| {
                                    on_item_toggle((enabled, index));
                                },
                                value: action.enabled,
                            }
                        }
                    }
                }
            }
            Button {
                text: "Add action",
                kind: ButtonKind::Secondary,
                on_click: move |_| {
                    on_add_click(());
                },
                class: "label mt-2",
            }
        }
    }
}

#[component]
fn ActionConfigurationItem(action: ActionConfiguration) -> Element {
    const ITEM_TEXT_CLASS: &str =
        "text-center inline-block pt-1 text-ellipsis overflow-hidden whitespace-nowrap";
    const ITEM_BORDER_CLASS: &str = "border-r-2 border-gray-700";

    let ActionConfiguration {
        key,
        link_key,
        count,
        condition,
        with,
        wait_before_millis,
        wait_after_millis,
        ..
    } = action;

    let linked_action = if matches!(condition, ActionConfigurationCondition::Linked) {
        ""
    } else {
        "mt-2"
    };
    let link_key = match link_key {
        Some(LinkKeyBinding::Before(key)) => format!("{key} ↝ "),
        Some(LinkKeyBinding::After(key)) => format!("{key} ↜ "),
        Some(LinkKeyBinding::AtTheSame(key)) => format!("{key} ↭ "),
        Some(LinkKeyBinding::Along(key)) => format!("{key} ↷ "),
        None => "".to_string(),
    };
    let millis = if let ActionConfigurationCondition::EveryMillis(millis) = condition {
        format!("⟳ {:.2}s / ", millis as f32 / 1000.0)
    } else {
        "".to_string()
    };
    let wait_before_secs = if wait_before_millis > 0 {
        Some(format!("⏱︎ {:.2}s", wait_before_millis as f32 / 1000.0))
    } else {
        None
    };
    let wait_after_secs = if wait_after_millis > 0 {
        Some(format!("⏱︎ {:.2}s", wait_after_millis as f32 / 1000.0))
    } else {
        None
    };
    let wait_secs = match (wait_before_secs, wait_after_secs) {
        (Some(before), None) => format!("{before} - ⏱︎ 0.00s / "),
        (None, None) => "".to_string(),
        (None, Some(after)) => format!("⏱︎ 0.00s - {after} / "),
        (Some(before), Some(after)) => format!("{before} - {after} / "),
    };
    let with = match with {
        ActionKeyWith::Any => "Any",
        ActionKeyWith::Stationary => "Stationary",
        ActionKeyWith::DoubleJump => "Double jump",
    };

    rsx! {
        div { class: "grid grid-cols-[100px_auto] h-6 paragraph-xs !text-gray-400 group-hover:bg-gray-900 {linked_action}",
            div { class: "{ITEM_BORDER_CLASS} {ITEM_TEXT_CLASS}", "{link_key}{key} × {count}" }
            div { class: "pr-13 {ITEM_TEXT_CLASS}", "{millis}{wait_secs}{with}" }
        }
    }
}
