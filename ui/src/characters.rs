use std::fmt::Display;

use backend::{
    ActionConfiguration, ActionConfigurationCondition, ActionKeyWith, Character, Class,
    IntoEnumIterator, KeyBinding, KeyBindingConfiguration, LinkKeyBinding, PotionMode,
    delete_character, query_characters, update_character, upsert_character,
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
enum CharacterUpdate {
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
    let mut character = use_context::<AppState>().character;
    let mut characters = use_resource(move || async move {
        spawn_blocking(|| query_characters().expect("failed to query characters"))
            .await
            .unwrap()
    });
    // Maps queried `characters` to names
    let character_names = use_memo(move || {
        characters()
            .unwrap_or_default()
            .into_iter()
            .map(|character| character.name)
            .collect()
    });
    // Maps currently selected `character` to the index in `characters`
    let character_index = use_memo(move || {
        characters()
            .zip(character())
            .and_then(|(characters, character)| {
                characters
                    .into_iter()
                    .enumerate()
                    .find(|(_, cfg)| character.id == cfg.id)
                    .map(|(i, _)| i)
            })
    });
    // Default character if `character` is `None`
    let character_view = use_memo(move || character().unwrap_or_default());

    // Handles async operations for character-related
    let coroutine = use_coroutine(
        move |mut rx: UnboundedReceiver<CharacterUpdate>| async move {
            let mut save_character = async move |current_character: Character| {
                let mut save_character = current_character.clone();
                spawn_blocking(move || {
                    upsert_character(&mut save_character)
                        .expect("failed to upsert character actions");
                })
                .await
                .unwrap();
                character.set(Some(current_character));
                characters.restart();
            };

            while let Some(message) = rx.next().await {
                match message {
                    CharacterUpdate::Set => {
                        if let Some(character) = character() {
                            update_character(character).await;
                        }
                    }
                    CharacterUpdate::Save => {
                        let Some(mut character) = character() else {
                            continue;
                        };
                        debug_assert!(character.id.is_some(), "saving invalid character");

                        spawn_blocking(move || {
                            upsert_character(&mut character).unwrap();
                        })
                        .await
                        .unwrap();
                    }
                    CharacterUpdate::Create(name) => {
                        let mut new_character = Character {
                            name,
                            ..Character::default()
                        };
                        let mut save_character = new_character.clone();
                        let save_id = spawn_blocking(move || {
                            upsert_character(&mut save_character).unwrap();
                            save_character
                                .id
                                .expect("character id must be valid after creation")
                        })
                        .await
                        .unwrap();

                        new_character.id = Some(save_id);
                        character.set(Some(new_character));
                        characters.restart();
                    }
                    CharacterUpdate::Delete => {
                        if let Some(character) = character.take() {
                            spawn_blocking(move || {
                                delete_character(&character).expect("failed to delete character");
                            })
                            .await
                            .unwrap();
                            characters.restart();
                        }
                    }
                    CharacterUpdate::AddAction(action) => {
                        let Some(mut character) = character() else {
                            continue;
                        };

                        character.actions.push(action);
                        save_character(character).await;
                    }
                    CharacterUpdate::EditAction(action, index) => {
                        let Some(mut character) = character() else {
                            continue;
                        };

                        *character.actions.get_mut(index).unwrap() = action;
                        save_character(character).await;
                    }
                    CharacterUpdate::DeleteAction(index) => {
                        let Some(mut character) = character() else {
                            continue;
                        };

                        character.actions.remove(index);
                        save_character(character).await;
                    }
                    CharacterUpdate::ToggleAction(enabled, index) => {
                        let Some(mut character) = character() else {
                            continue;
                        };

                        let character_mut = character.actions.get_mut(index).unwrap();
                        character_mut.enabled = enabled;
                        save_character(character).await;
                    }
                }
            }
        },
    );
    let save_character = use_callback(move |new_character: Character| {
        character.set(Some(new_character));
        coroutine.send(CharacterUpdate::Save);
        coroutine.send(CharacterUpdate::Set);
    });
    let action_input_kind = use_signal(|| None);

    // Sets a character if there is not one
    use_effect(move || {
        if let Some(characters) = characters()
            && !characters.is_empty()
            && character.peek().is_none()
        {
            character.set(characters.into_iter().next());
            coroutine.send(CharacterUpdate::Set);
        }
    });

    rsx! {
        div { class: "flex flex-col pb-15 h-full overflow-y-auto scrollbar",
            SectionKeyBindings { character_view, save_character }
            SectionBuffs { character_view, save_character }
            SectionFixedActions {
                action_input_kind,
                character_view,
                save_character,
            }
            SectionOthers { character_view, save_character }
        }
        PopupActionConfigurationInput { action_input_kind, actions: character_view().actions }
        div { class: "flex items-center w-full h-10 bg-gray-950 absolute bottom-0 pr-2",
            TextSelect {
                class: "flex-grow",
                options: character_names(),
                disabled: false,
                placeholder: "Create a character...",
                on_create: move |name| {
                    coroutine.send(CharacterUpdate::Create(name));
                    coroutine.send(CharacterUpdate::Set);
                },
                on_delete: move |_| {
                    coroutine.send(CharacterUpdate::Delete);
                },
                on_select: move |(index, _)| {
                    let selected = characters.peek().as_ref().unwrap().get(index).cloned().unwrap();
                    character.set(Some(selected));
                    coroutine.send(CharacterUpdate::Set);
                },
                selected: character_index(),
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
    character_view: Memo<Character>,
    save_character: Callback<Character>,
) -> Element {
    rsx! {
        Section { name: "Key bindings",
            div { class: "grid grid-cols-2 gap-4",
                KeyBindingConfigurationInput {
                    label: "Rope lift",
                    optional: true,
                    disabled: character_view().id.is_none(),
                    on_value: move |ropelift_key| {
                        save_character(Character {
                            ropelift_key,
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().ropelift_key,
                }
                KeyBindingConfigurationInput {
                    label: "Teleport",
                    optional: true,
                    disabled: character_view().id.is_none(),
                    on_value: move |teleport_key| {
                        save_character(Character {
                            teleport_key,
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().teleport_key,
                }
                KeyBindingConfigurationInput {
                    label: "Jump",
                    disabled: character_view().id.is_none(),
                    on_value: move |key_config: Option<KeyBindingConfiguration>| {
                        save_character(Character {
                            jump_key: key_config.expect("not optional"),
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().jump_key,
                }
                KeyBindingConfigurationInput {
                    label: "Up jump",
                    optional: true,
                    disabled: character_view().id.is_none(),
                    on_value: move |up_jump_key| {
                        save_character(Character {
                            up_jump_key,
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().up_jump_key,
                }
                KeyBindingConfigurationInput {
                    label: "Interact",
                    disabled: character_view().id.is_none(),
                    on_value: move |key_config: Option<KeyBindingConfiguration>| {
                        save_character(Character {
                            interact_key: key_config.expect("not optional"),
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().interact_key,
                }
                KeyBindingConfigurationInput {
                    label: "Cash shop",
                    disabled: character_view().id.is_none(),
                    on_value: move |key_config: Option<KeyBindingConfiguration>| {
                        save_character(Character {
                            cash_shop_key: key_config.expect("not optional"),
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().cash_shop_key,
                }
                KeyBindingConfigurationInput {
                    label: "Maple guide",
                    disabled: character_view().id.is_none(),
                    on_value: move |key_config: Option<KeyBindingConfiguration>| {
                        save_character(Character {
                            maple_guide_key: key_config.expect("not optional"),
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().maple_guide_key,
                }
                KeyBindingConfigurationInput {
                    label: "Change channel",
                    disabled: character_view().id.is_none(),
                    on_value: move |key_config: Option<KeyBindingConfiguration>| {
                        save_character(Character {
                            change_channel_key: key_config.expect("not optional"),
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().change_channel_key,
                }
                KeyBindingConfigurationInput {
                    label: "Feed pet",
                    disabled: character_view().id.is_none(),
                    on_value: move |key_config: Option<KeyBindingConfiguration>| {
                        save_character(Character {
                            feed_pet_key: key_config.expect("not optional"),
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().feed_pet_key,
                }
                KeyBindingConfigurationInput {
                    label: "Potion",
                    disabled: character_view().id.is_none(),
                    on_value: move |key_config: Option<KeyBindingConfiguration>| {
                        save_character(Character {
                            potion_key: key_config.expect("not optional"),
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().potion_key,
                }
                div { class: "col-span-full grid-cols-3 grid gap-2 justify-items-stretch",
                    KeyBindingConfigurationInput {
                        label: "Familiar menu",
                        disabled: character_view().id.is_none(),
                        on_value: move |key_config: Option<KeyBindingConfiguration>| {
                            save_character(Character {
                                familiar_menu_key: key_config.expect("not optional"),
                                ..character_view.peek().clone()
                            });
                        },
                        value: character_view().familiar_menu_key,
                    }
                    KeyBindingConfigurationInput {
                        label: "Familiar skill",
                        disabled: character_view().id.is_none(),
                        on_value: move |key_config: Option<KeyBindingConfiguration>| {
                            save_character(Character {
                                familiar_buff_key: key_config.expect("not optional"),
                                ..character_view.peek().clone()
                            });
                        },
                        value: character_view().familiar_buff_key,
                    }
                    KeyBindingConfigurationInput {
                        label: "Familiar essence",
                        disabled: character_view().id.is_none(),
                        on_value: move |key_config: Option<KeyBindingConfiguration>| {
                            save_character(Character {
                                familiar_essence_key: key_config.expect("not optional"),
                                ..character_view.peek().clone()
                            });
                        },
                        value: character_view().familiar_essence_key,
                    }
                }
            }
        }
    }
}

#[component]
fn SectionBuffs(character_view: Memo<Character>, save_character: Callback<Character>) -> Element {
    #[component]
    fn Buff(
        label: &'static str,
        disabled: bool,
        on_value: EventHandler<KeyBindingConfiguration>,
        value: KeyBindingConfiguration,
    ) -> Element {
        rsx! {
            div { class: "grid grid-cols-[140px_auto] gap-2",
                KeyBindingConfigurationInput {
                    label,
                    disabled,
                    on_value: move |config: Option<KeyBindingConfiguration>| {
                        on_value(config.expect("not optional"));
                    },
                    value: Some(value),
                }
                Checkbox {
                    label: "Enabled",
                    disabled,
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
                Checkbox {
                    label: "Familiar essence and skill",
                    disabled: character_view().id.is_none(),
                    on_value: move |enabled| {
                        let character = character_view.peek().clone();
                        save_character(Character {
                            familiar_buff_key: KeyBindingConfiguration {
                                enabled,
                                ..character.familiar_buff_key
                            },
                            ..character
                        });
                    },
                    value: character_view().familiar_buff_key.enabled,
                    input_class: "w-6",
                }
                div {}
                Buff {
                    label: "Sayram's Elixir",
                    disabled: character_view().id.is_none(),
                    on_value: move |sayram_elixir_key| {
                        save_character(Character {
                            sayram_elixir_key,
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().sayram_elixir_key,
                }
                Buff {
                    label: "Aurelia's Elixir",
                    disabled: character_view().id.is_none(),
                    on_value: move |aurelia_elixir_key| {
                        save_character(Character {
                            aurelia_elixir_key,
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().aurelia_elixir_key,
                }
                Buff {
                    label: "3x EXP Coupon",
                    disabled: character_view().id.is_none(),
                    on_value: move |exp_x3_key| {
                        save_character(Character {
                            exp_x3_key,
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().exp_x3_key,
                }
                Buff {
                    label: "50% Bonus EXP Coupon",
                    disabled: character_view().id.is_none(),
                    on_value: move |bonus_exp_key| {
                        save_character(Character {
                            bonus_exp_key,
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().bonus_exp_key,
                }
                Buff {
                    label: "Legion's Wealth",
                    disabled: character_view().id.is_none(),
                    on_value: move |legion_wealth_key| {
                        save_character(Character {
                            legion_wealth_key,
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().legion_wealth_key,
                }
                Buff {
                    label: "Legion's Luck",
                    disabled: character_view().id.is_none(),
                    on_value: move |legion_luck_key| {
                        save_character(Character {
                            legion_luck_key,
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().legion_luck_key,
                }
                Buff {
                    label: "Wealth Acquisition Potion",
                    disabled: character_view().id.is_none(),
                    on_value: move |wealth_acquisition_potion_key| {
                        save_character(Character {
                            wealth_acquisition_potion_key,
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().wealth_acquisition_potion_key,
                }
                Buff {
                    label: "EXP Accumulation Potion",
                    disabled: character_view().id.is_none(),
                    on_value: move |exp_accumulation_potion_key| {
                        save_character(Character {
                            exp_accumulation_potion_key,
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().exp_accumulation_potion_key,
                }
                Buff {
                    label: "Extreme Red Potion",
                    disabled: character_view().id.is_none(),
                    on_value: move |extreme_red_potion_key| {
                        save_character(Character {
                            extreme_red_potion_key,
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().extreme_red_potion_key,
                }
                Buff {
                    label: "Extreme Blue Potion",
                    disabled: character_view().id.is_none(),
                    on_value: move |extreme_blue_potion_key| {
                        save_character(Character {
                            extreme_blue_potion_key,
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().extreme_blue_potion_key,
                }
                Buff {
                    label: "Extreme Green Potion",
                    disabled: character_view().id.is_none(),
                    on_value: move |extreme_green_potion_key| {
                        save_character(Character {
                            extreme_green_potion_key,
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().extreme_green_potion_key,
                }
                Buff {
                    label: "Extreme Gold Potion",
                    disabled: character_view().id.is_none(),
                    on_value: move |extreme_gold_potion_key| {
                        save_character(Character {
                            extreme_gold_potion_key,
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().extreme_gold_potion_key,
                }
            }
        }
    }
}

#[component]
fn SectionFixedActions(
    action_input_kind: Signal<Option<ActionConfigurationInputKind>>,
    character_view: Memo<Character>,
    save_character: Callback<Character>,
) -> Element {
    let coroutine = use_coroutine_handle::<CharacterUpdate>();

    rsx! {
        Section { name: "Fixed actions",
            ActionConfigurationList {
                disabled: character_view().id.is_none(),
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
                    coroutine.send(CharacterUpdate::DeleteAction(index));
                    coroutine.send(CharacterUpdate::Set);
                },
                on_item_toggle: move |(enabled, index)| {
                    coroutine.send(CharacterUpdate::ToggleAction(enabled, index));
                    coroutine.send(CharacterUpdate::Set);
                },
                actions: character_view().actions,
            }
        }
    }
}

#[component]
fn SectionOthers(character_view: Memo<Character>, save_character: Callback<Character>) -> Element {
    rsx! {
        Section { name: "Others",
            div { class: "grid grid-cols-2 gap-4",
                CharactersMillisInput {
                    label: "Feed pet every milliseconds",
                    disabled: character_view().id.is_none(),
                    on_value: move |feed_pet_millis| {
                        save_character(Character {
                            feed_pet_millis,
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().feed_pet_millis,
                }
                div {} // Spacer

                CharactersSelect::<PotionMode> {
                    label: "Use potion mode",
                    disabled: character_view().id.is_none(),
                    on_select: move |potion_mode| {
                        save_character(Character {
                            potion_mode,
                            ..character_view.peek().clone()
                        });
                    },
                    selected: character_view().potion_mode,
                }
                match character_view().potion_mode {
                    PotionMode::EveryMillis(millis) => rsx! {
                        CharactersMillisInput {
                            label: "Use potion every milliseconds",
                            disabled: character_view().id.is_none(),
                            on_value: move |millis| {
                                save_character(Character {
                                    potion_mode: PotionMode::EveryMillis(millis),
                                    ..character_view.peek().clone()
                                });
                            },
                            value: millis,
                        }
                    },
                    PotionMode::Percentage(percent) => rsx! {
                        CharactersPercentageInput {
                            label: "Use potion health below percentage",
                            disabled: character_view().id.is_none(),
                            on_value: move |percent| {
                                save_character(Character {
                                    potion_mode: PotionMode::Percentage(percent),
                                    ..character_view.peek().clone()
                                });
                            },
                            value: percent,
                        }
                    },
                }

                CharactersSelect::<Class> {
                    label: "Link key timing class",
                    disabled: character_view().id.is_none(),
                    on_select: move |class| {
                        save_character(Character {
                            class,
                            ..character_view.peek().clone()
                        });
                    },
                    selected: character_view().class,
                }
                CharactersCheckbox {
                    label: "Disable walking",
                    disabled: character_view().id.is_none(),
                    on_value: move |disable_adjusting| {
                        save_character(Character {
                            disable_adjusting,
                            ..character_view.peek().clone()
                        });
                    },
                    value: character_view().disable_adjusting,
                }
            }
        }
    }
}

#[component]
fn KeyBindingConfigurationInput(
    label: &'static str,
    #[props(default = false)] optional: bool,
    disabled: bool,
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
            disabled,
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
    #[props(default = false)] disabled: bool,
    on_value: EventHandler<bool>,
    value: bool,
) -> Element {
    rsx! {
        Checkbox {
            label,
            label_class,
            input_class: "w-6",
            disabled,
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
    disabled: bool,
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
    let coroutine = use_coroutine_handle::<CharacterUpdate>();
    let save_action = use_callback(move |action| {
        let update = match action_input_kind
            .take()
            .expect("input kind must already be set")
        {
            ActionConfigurationInputKind::Add(_) => CharacterUpdate::AddAction(action),
            ActionConfigurationInputKind::Edit(_, index) => {
                CharacterUpdate::EditAction(action, index)
            }
        };
        coroutine.send(update);
        coroutine.send(CharacterUpdate::Set);
    });

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
                                save_action(action);
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
    disabled: bool,
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
                disabled,
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
