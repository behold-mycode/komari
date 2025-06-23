use std::{
    fmt::Display,
    mem::{discriminant, swap},
    ops::Range,
};

use backend::{
    Action, ActionCondition, ActionKey, ActionKeyDirection, ActionKeyWith, ActionMove,
    IntoEnumIterator, KeyBinding, LinkKeyBinding, Minimap, Position, update_minimap, upsert_map,
};
use dioxus::prelude::*;
use futures_util::StreamExt;
use tokio::task::spawn_blocking;

use crate::{
    AppState,
    button::{Button, ButtonKind},
    icons::{DownArrowIcon, UpArrowIcon, XIcon},
    inputs::{Checkbox, KeyBindingInput, MillisInput, NumberInputI32, NumberInputU32},
    select::{EnumSelect, TextSelect},
};

const INPUT_CLASS: &str = "h-6 w-full";
const ACTION_ITEM_TEXT_CLASS: &str =
    "text-center inline-block text-ellipsis overflow-hidden whitespace-nowrap";
const ACTION_ITEM_BORDER_CLASS: &str = "border-r-2 border-gray-700";

#[derive(Debug)]
enum ActionUpdate {
    SetPreset,
    CreatePreset(String),
    DeletePreset,
    Add(Action, ActionCondition),
    Edit(Action, usize),
    Delete(usize),
    Move(usize, ActionCondition, bool),
}

#[derive(Clone, Copy, Debug)]
enum ActionInputKind {
    Add(Action),
    Edit(Action, usize),
}

#[component]
pub fn Actions() -> Element {
    let mut minimap = use_context::<AppState>().minimap;
    let mut minimap_preset = use_context::<AppState>().minimap_preset;
    let minimap_presets = use_memo(move || {
        minimap()
            .map(|minimap| minimap.actions.into_keys().collect::<Vec<String>>())
            .unwrap_or_default()
    });
    // Maps currently selected `minimap_preset` to actions
    let minimap_preset_actions = use_memo(move || {
        minimap()
            .zip(minimap_preset())
            .and_then(|(minimap, preset)| minimap.actions.get(&preset).cloned())
            .unwrap_or_default()
    });
    // Maps currently selected `minimap_preset` to the index in `minimap_presets`
    let minimap_preset_index = use_memo(move || {
        let presets = minimap_presets();
        minimap_preset().and_then(|preset| {
            presets
                .into_iter()
                .enumerate()
                .find(|(_, p)| &preset == p)
                .map(|(i, _)| i)
        })
    });

    // Handles async operations for action-related
    let coroutine = use_coroutine(move |mut rx: UnboundedReceiver<ActionUpdate>| async move {
        let mut save_minimap = async move |current_minimap: Minimap| {
            let mut save_minimap = current_minimap.clone();
            spawn_blocking(move || {
                upsert_map(&mut save_minimap).expect("failed to upsert minimap actions");
            })
            .await
            .unwrap();
            minimap.set(Some(current_minimap));
        };

        while let Some(message) = rx.next().await {
            match message {
                ActionUpdate::SetPreset => {
                    if let Some(minimap) = minimap() {
                        update_minimap(minimap_preset(), minimap).await;
                    }
                }
                ActionUpdate::CreatePreset(preset) => {
                    let Some(mut current_minimap) = minimap() else {
                        continue;
                    };

                    if current_minimap.actions.try_insert(preset, vec![]).is_ok() {
                        save_minimap(current_minimap).await;
                    }
                }
                ActionUpdate::DeletePreset => {
                    let Some(mut current_minimap) = minimap() else {
                        continue;
                    };
                    let Some(preset) = minimap_preset() else {
                        continue;
                    };

                    if current_minimap.actions.remove(&preset).is_some() {
                        minimap_preset.set(None);
                        save_minimap(current_minimap).await;
                    }
                }
                ActionUpdate::Add(action, condition) => {
                    let Some(mut current_minimap) = minimap() else {
                        continue;
                    };
                    let Some(preset) = minimap_preset() else {
                        continue;
                    };
                    let Some(actions) = current_minimap.actions.get_mut(&preset) else {
                        continue;
                    };
                    let index = if matches!(action.condition(), ActionCondition::Linked) {
                        find_last_linked_action_index(actions, condition)
                            .map(|index| index + 1)
                            .unwrap_or(actions.len())
                    } else {
                        actions.len()
                    };

                    actions.insert(index, action);
                    save_minimap(current_minimap).await;
                }
                ActionUpdate::Edit(action, index) => {
                    let Some(mut current_minimap) = minimap() else {
                        continue;
                    };
                    let Some(preset) = minimap_preset() else {
                        continue;
                    };
                    let Some(actions) = current_minimap.actions.get_mut(&preset) else {
                        continue;
                    };

                    actions[index] = action;
                    save_minimap(current_minimap).await;
                }
                ActionUpdate::Delete(index) => {
                    let Some(mut current_minimap) = minimap() else {
                        continue;
                    };
                    let Some(preset) = minimap_preset() else {
                        continue;
                    };
                    let Some(actions) = current_minimap.actions.get_mut(&preset) else {
                        continue;
                    };
                    let action = actions[index];

                    // Replaces the first linked action to this `action` condition
                    // TODO: Maybe replace find_linked_action_range with a simple lookahead
                    if !matches!(action.condition(), ActionCondition::Linked)
                        && find_linked_action_range(actions, index).is_some()
                    {
                        actions[index + 1] = actions[index + 1].with_condition(action.condition());
                    }
                    actions.remove(index);
                    save_minimap(current_minimap).await;
                }
                ActionUpdate::Move(index, condition, up) => {
                    let Some(mut current_minimap) = minimap() else {
                        continue;
                    };
                    let Some(preset) = minimap_preset() else {
                        continue;
                    };
                    let Some(actions) = current_minimap.actions.get_mut(&preset) else {
                        continue;
                    };
                    let filtered = filter_actions(actions.clone(), condition);
                    if (up && index <= filtered.first().expect("cannot be empty").1)
                        || (!up && index >= filtered.last().expect("cannot be empty").1)
                    {
                        continue;
                    }

                    // Finds the action index of `filtered` before or after `index`
                    let filtered_index = filtered
                        .iter()
                        .enumerate()
                        .find_map(|(filtered_index, (_, actions_index))| {
                            if *actions_index == index {
                                if up {
                                    Some(filtered_index - 1)
                                } else {
                                    Some(filtered_index + 1)
                                }
                            } else {
                                None
                            }
                        })
                        .expect("must be valid index");
                    let filtered_condition = filtered[filtered_index].0.condition();
                    let action_condition = actions[index].condition();
                    match (action_condition, filtered_condition) {
                        // Simple case - swapping two linked actions
                        (ActionCondition::Linked, ActionCondition::Linked) => {
                            actions.swap(index, filtered[filtered_index].1);
                            save_minimap(current_minimap).await;
                            continue;
                        }
                        // Disallows moving up/down if `index` is a linked action and
                        // `filtered_index` is a non-linked action
                        (ActionCondition::Linked, _) => continue,
                        _ => (),
                    }

                    // Finds the first non-linked action index of `filtered` before or after `index`
                    let mut filtered_non_linked_index = filtered_index;
                    while (up && filtered_non_linked_index > 0)
                        || (!up && filtered_non_linked_index < filtered.len() - 1)
                    {
                        let condition = filtered[filtered_non_linked_index].0.condition();
                        if !matches!(condition, ActionCondition::Linked) {
                            break;
                        }
                        if up {
                            filtered_non_linked_index -= 1;
                        } else {
                            filtered_non_linked_index += 1;
                        }
                    }
                    let condition = filtered[filtered_non_linked_index].0.condition();
                    if matches!(condition, ActionCondition::Linked) {
                        continue;
                    }

                    let actions_non_linked_index = filtered[filtered_non_linked_index].1;
                    let first_range = find_linked_action_range(actions, actions_non_linked_index);
                    let mut first_range = if let Some(range) = first_range {
                        actions_non_linked_index..range.end
                    } else {
                        actions_non_linked_index..actions_non_linked_index + 1
                    };

                    let second_range = find_linked_action_range(actions, index);
                    let mut second_range = if let Some(range) = second_range {
                        index..range.end
                    } else {
                        index..index + 1
                    };

                    if !up {
                        swap(&mut first_range, &mut second_range);
                    }

                    debug_assert!(
                        first_range.end <= second_range.start
                            || second_range.end <= first_range.start
                    );
                    let second_start = second_range.start;
                    let second_actions = actions.drain(second_range).collect::<Vec<_>>();
                    let first_actions = actions[first_range.clone()].to_vec();
                    for action in first_actions.into_iter().rev() {
                        actions.insert(second_start, action);
                    }

                    let first_start = first_range.start;
                    let _ = actions.drain(first_range);
                    for action in second_actions.into_iter().rev() {
                        actions.insert(first_start, action);
                    }

                    save_minimap(current_minimap).await;
                }
            }
        }
    });
    let mut action_input_kind = use_signal(|| None);
    let actions_list_disabled = use_memo(move || minimap().is_none() || minimap_preset().is_none());

    // Sets a preset if there is not one
    use_effect(move || {
        if let Some(minimap) = minimap() {
            if !minimap.actions.is_empty() && minimap_preset.peek().is_none() {
                minimap_preset.set(minimap.actions.into_keys().next());
                coroutine.send(ActionUpdate::SetPreset);
            }
        } else {
            minimap_preset.set(None);
            coroutine.send(ActionUpdate::SetPreset);
        }
    });

    rsx! {
        div { class: "flex flex-col pb-15 h-full gap-3 overflow-y-auto scrollbar pr-2",
            SectionLegends {}
            Section { name: "Normal actions",
                ActionList {
                    on_add_click: move |_| {
                        action_input_kind
                            .set(Some(ActionInputKind::Add(Action::Key(ActionKey::default()))));
                    },
                    on_item_click: move |(action, index)| {
                        action_input_kind.set(Some(ActionInputKind::Edit(action, index)));
                    },
                    on_item_move: move |(index, condition, up)| {
                        coroutine.send(ActionUpdate::Move(index, condition, up));
                        coroutine.send(ActionUpdate::SetPreset);
                    },
                    on_item_delete: move |index| {
                        coroutine.send(ActionUpdate::Delete(index));
                        coroutine.send(ActionUpdate::SetPreset);
                    },
                    condition_filter: ActionCondition::Any,
                    disabled: actions_list_disabled(),
                    actions: minimap_preset_actions(),
                }
            }
            Section { name: "Erda Shower off cooldown priority actions",
                ActionList {
                    on_add_click: move |_| {
                        let action = Action::Key(ActionKey {
                            condition: ActionCondition::ErdaShowerOffCooldown,
                            ..ActionKey::default()
                        });
                        action_input_kind.set(Some(ActionInputKind::Add(action)));
                    },
                    on_item_click: move |(action, index)| {
                        action_input_kind.set(Some(ActionInputKind::Edit(action, index)));
                    },
                    on_item_move: move |(index, condition, up)| {
                        coroutine.send(ActionUpdate::Move(index, condition, up));
                        coroutine.send(ActionUpdate::SetPreset);
                    },
                    on_item_delete: move |index| {
                        coroutine.send(ActionUpdate::Delete(index));
                        coroutine.send(ActionUpdate::SetPreset);
                    },
                    condition_filter: ActionCondition::ErdaShowerOffCooldown,
                    disabled: actions_list_disabled(),
                    actions: minimap_preset_actions(),
                }
            }
            Section { name: "Every milliseconds priority actions",
                ActionList {
                    on_add_click: move |_| {
                        let action = Action::Key(ActionKey {
                            condition: ActionCondition::EveryMillis(0),
                            ..ActionKey::default()
                        });
                        action_input_kind.set(Some(ActionInputKind::Add(action)));
                    },
                    on_item_click: move |(action, index)| {
                        action_input_kind.set(Some(ActionInputKind::Edit(action, index)));
                    },
                    on_item_move: move |(index, condition, up)| {
                        coroutine.send(ActionUpdate::Move(index, condition, up));
                        coroutine.send(ActionUpdate::SetPreset);
                    },
                    on_item_delete: move |index| {
                        coroutine.send(ActionUpdate::Delete(index));
                        coroutine.send(ActionUpdate::SetPreset);
                    },
                    condition_filter: ActionCondition::EveryMillis(0),
                    disabled: actions_list_disabled(),
                    actions: minimap_preset_actions(),
                }
            }
        }
        PopupActionInput { action_input_kind, actions: minimap_preset_actions }
        div { class: "flex items-center w-full h-10 pr-2 bg-gray-950 absolute bottom-0",
            TextSelect {
                class: "h-6 flex-grow",
                options: minimap_presets(),
                disabled: minimap().is_none(),
                placeholder: "Create a preset...",
                on_create: move |name| {
                    coroutine.send(ActionUpdate::CreatePreset(name));
                    coroutine.send(ActionUpdate::SetPreset);
                },
                on_delete: move |_| {
                    coroutine.send(ActionUpdate::DeletePreset);
                    coroutine.send(ActionUpdate::SetPreset);
                },
                on_select: move |(_, preset)| {
                    minimap_preset.set(Some(preset));
                    coroutine.send(ActionUpdate::SetPreset);
                },
                selected: minimap_preset_index(),
            }
        }
    }
}

#[component]
fn Section(
    name: String,
    #[props(default = String::default())] class: String,
    children: Element,
) -> Element {
    rsx! {
        div { class: "flex flex-col gap-2 {class}",
            div { class: "flex items-center title-xs h-10", {name} }
            {children}
        }
    }
}

#[component]
fn SectionLegends() -> Element {
    rsx! {
        Section { name: "Legends" }
    }
}

#[component]
fn PopupActionInput(
    action_input_kind: Signal<Option<ActionInputKind>>,
    actions: ReadOnlySignal<Vec<Action>>,
) -> Element {
    #[derive(PartialEq, Clone, Copy, Debug)]
    struct State {
        action: Action,
        modifying: bool,
        can_create_linked_action: bool,
    }

    let state = use_memo(move || {
        action_input_kind().map(|kind| {
            let actions = actions();
            let (action, index) = match kind {
                ActionInputKind::Add(action) => (action, None),
                ActionInputKind::Edit(action, index) => (action, Some(index)),
            };
            let modifying = matches!(kind, ActionInputKind::Edit(_, _));
            let can_create_linked_action = match action.condition() {
                ActionCondition::EveryMillis(_)
                | ActionCondition::ErdaShowerOffCooldown
                | ActionCondition::Any => {
                    let filtered = filter_actions(actions, action.condition());
                    let is_not_empty = !filtered.is_empty();
                    let first_index = filtered.into_iter().next().map(|first| first.1);

                    is_not_empty && first_index != index
                }
                ActionCondition::Linked => false,
            };

            State {
                action,
                modifying,
                can_create_linked_action,
            }
        })
    });
    let coroutine = use_coroutine_handle::<ActionUpdate>();

    rsx! {
        if let Some(State { action, modifying, can_create_linked_action }) = state() {
            div { class: "p-8 w-full h-full absolute inset-0 z-1 bg-gray-950/80",
                ActionInput {
                    modifying,
                    can_create_linked_action,
                    on_cancel: move |_| {
                        action_input_kind.set(None);
                    },
                    on_value: move |(action, condition)| {
                        match action_input_kind.take().expect("input kind must already be set") {
                            ActionInputKind::Add(_) => {
                                coroutine.send(ActionUpdate::Add(action, condition));
                            }
                            ActionInputKind::Edit(_, index) => {
                                coroutine.send(ActionUpdate::Edit(action, index));
                            }
                        }
                        coroutine.send(ActionUpdate::SetPreset);
                    },
                    value: action,
                }
            }
        }
    }
}

#[component]
fn ActionInput(
    modifying: bool,
    can_create_linked_action: bool,
    on_cancel: EventHandler,
    on_value: EventHandler<(Action, ActionCondition)>,
    value: Action,
) -> Element {
    let action_name = match value.condition() {
        backend::ActionCondition::Any => "normal",
        backend::ActionCondition::EveryMillis(_) => "every milliseconds",
        backend::ActionCondition::ErdaShowerOffCooldown => "Erda Shower off cooldown",
        backend::ActionCondition::Linked => "linked",
    };
    let mut action = use_signal(use_reactive!(|value| value));
    let title = if modifying {
        format!("Modify a {action_name} action")
    } else {
        format!("Add a new {action_name} action")
    };
    let button_text = use_memo(move || {
        if matches!(action(), Action::Move(_)) {
            "Switch to key"
        } else {
            "Switch to move"
        }
    });

    rsx! {
        div { class: "bg-gray-900 h-full px-2",
            Section { name: title, class: "relative h-full",
                Button {
                    text: button_text(),
                    kind: ButtonKind::Primary,
                    on_click: move |_| {
                        if discriminant(&value) != discriminant(&*action.peek()) {
                            action.set(value);
                        } else if matches!(value, Action::Move(_)) {
                            action
                                .set(
                                    Action::Key(ActionKey {
                                        condition: value.condition(),
                                        ..ActionKey::default()
                                    }),
                                );
                        } else {
                            action
                                .set(
                                    Action::Move(ActionMove {
                                        condition: value.condition(),
                                        ..ActionMove::default()
                                    }),
                                );
                        }
                    },
                    class: "h-5 label border-b border-gray-600",
                }
                match action() {
                    Action::Move(action) => rsx! {
                        ActionMoveInput {
                            modifying,
                            can_create_linked_action,
                            on_cancel,
                            on_value: move |(action, condition)| {
                                on_value((Action::Move(action), condition));
                            },
                            value: action,
                        }
                    },
                    Action::Key(action) => rsx! {
                        ActionKeyInput {
                            modifying,
                            can_create_linked_action,
                            on_cancel,
                            on_value: move |(action, condition)| {
                                on_value((Action::Key(action), condition));
                            },
                            value: action,
                        }
                    },
                }
            }
        }
    }
}

#[component]
fn ActionMoveInput(
    modifying: bool,
    can_create_linked_action: bool,
    on_cancel: EventHandler,
    on_value: EventHandler<(ActionMove, ActionCondition)>,
    value: ActionMove,
) -> Element {
    let mut action = use_signal(|| value);

    use_effect(use_reactive!(|value| { action.set(value) }));

    rsx! {
        div { class: "grid grid-cols-3 gap-3",
            // Position
            ActionsNumberInputI32 {
                label: "X",
                on_value: move |x| {
                    let mut action = action.write();
                    action.position.x = x;
                },
                value: action().position.x,
            }
            ActionsNumberInputI32 {
                label: "Y",
                on_value: move |y| {
                    let mut action = action.write();
                    action.position.y = y;
                },
                value: action().position.y,
            }
            ActionsCheckbox {
                label: "Adjust",
                on_value: move |adjust: bool| {
                    let mut action = action.write();
                    action.position.allow_adjusting = adjust;
                },
                value: action().position.allow_adjusting,
            }

            ActionsMillisInput {
                label: "Wait after move",
                on_value: move |millis| {
                    let mut action = action.write();
                    action.wait_after_move_millis = millis;
                },
                value: action().wait_after_move_millis,
            }
            if can_create_linked_action {
                ActionsCheckbox {
                    label: "Linked action",
                    on_value: move |is_linked: bool| {
                        let mut action = action.write();
                        action.condition = if is_linked {
                            ActionCondition::Linked
                        } else {
                            value.condition
                        };
                    },
                    value: matches!(action().condition, ActionCondition::Linked),
                }
            }
        }
        div { class: "flex w-full gap-3 absolute bottom-2",
            Button {
                class: "h-6 flex-grow border border-gray-600",
                text: if modifying { "Save" } else { "Add" },
                kind: ButtonKind::Primary,
                on_click: move |_| {
                    on_value((*action.peek(), value.condition));
                },
            }
            Button {
                class: "flex-grow h-6 border border-gray-600",
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
fn ActionKeyInput(
    modifying: bool,
    can_create_linked_action: bool,
    on_cancel: EventHandler,
    on_value: EventHandler<(ActionKey, ActionCondition)>,
    value: ActionKey,
) -> Element {
    let mut action = use_signal(|| value);

    use_effect(use_reactive!(|value| { action.set(value) }));

    rsx! {
        div { class: "grid grid-cols-3 gap-3 pb-10 overflow-y-auto scrollbar",
            div { class: "col-span-3",
                ActionsCheckbox {
                    label: "Positioned",
                    on_value: move |has_position: bool| {
                        let mut action = action.write();
                        action.position = has_position.then_some(Position::default());
                    },
                    value: action().position.is_some(),
                }
            }


            // Position
            ActionsNumberInputI32 {
                label: "X",
                disabled: action().position.is_none(),
                on_value: move |x| {
                    let mut action = action.write();
                    action.position.as_mut().unwrap().x = x;
                },
                value: action().position.map(|pos| pos.x).unwrap_or_default(),
            }
            ActionsNumberInputI32 {
                label: "Y",
                disabled: action().position.is_none(),
                on_value: move |y| {
                    let mut action = action.write();
                    action.position.as_mut().unwrap().y = y;
                },
                value: action().position.map(|pos| pos.y).unwrap_or_default(),
            }
            ActionsCheckbox {
                label: "Adjust",
                disabled: action().position.is_none(),
                on_value: move |adjust: bool| {
                    let mut action = action.write();
                    action.position.as_mut().unwrap().allow_adjusting = adjust;
                },
                value: action().position.map(|pos| pos.allow_adjusting).unwrap_or_default(),
            }

            // Key, count and link key
            ActionsKeyBindingInput {
                label: "Key",
                disabled: false,
                on_value: move |key: Option<KeyBinding>| {
                    let mut action = action.write();
                    action.key = key.expect("not optional");
                },
                value: Some(action().key),
            }
            ActionsNumberInputU32 {
                label: "Use count",
                on_value: move |count| {
                    let mut action = action.write();
                    action.count = count;
                },
                value: action().count,
            }
            if can_create_linked_action {
                ActionsCheckbox {
                    label: "Linked action",
                    on_value: move |is_linked: bool| {
                        let mut action = action.write();
                        action.condition = if is_linked {
                            ActionCondition::Linked
                        } else {
                            value.condition
                        };
                    },
                    value: matches!(action().condition, ActionCondition::Linked),
                }
            } else {
                div {} // Spacer
            }
            ActionsKeyBindingInput {
                label: "Link key",
                disabled: action().link_key.is_none(),
                on_value: move |key: Option<KeyBinding>| {
                    let mut action = action.write();
                    action.link_key = action
                        .link_key
                        .map(|link_key| link_key.with_key(key.expect("not optional")));
                },
                value: action().link_key.unwrap_or_default().key(),
            }
            ActionsSelect::<LinkKeyBinding> {
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
            ActionsCheckbox {
                label: "Has link key",
                on_value: move |has_link_key: bool| {
                    let mut action = action.write();
                    action.link_key = has_link_key.then_some(LinkKeyBinding::default());
                },
                value: action().link_key.is_some(),
            }

            // Use with, direction
            ActionsSelect::<ActionKeyWith> {
                label: "Use key with",
                disabled: false,
                on_select: move |with| {
                    let mut action = action.write();
                    action.with = with;
                },
                selected: action().with,
            }
            ActionsSelect::<ActionKeyDirection> {
                label: "Use key direction",
                disabled: false,
                on_select: move |direction| {
                    let mut action = action.write();
                    action.direction = direction;
                },
                selected: action().direction,
            }
            if matches!(
                action().condition,
                ActionCondition::EveryMillis(_) | ActionCondition::ErdaShowerOffCooldown
            )
            {
                ActionsCheckbox {
                    label: "Queue to front",
                    on_value: move |queue_to_front: bool| {
                        let mut action = action.write();
                        action.queue_to_front = Some(queue_to_front);
                    },
                    value: action().queue_to_front.is_some(),
                }
            } else {
                div {} // Spacer
            }

            // Wait before use
            ActionsMillisInput {
                label: "Wait before",
                on_value: move |millis| {
                    let mut action = action.write();
                    action.wait_before_use_millis = millis;
                },
                value: action().wait_before_use_millis,
            }
            ActionsMillisInput {
                label: "Wait random range",
                on_value: move |millis| {
                    let mut action = action.write();
                    action.wait_before_use_millis_random_range = millis;
                },
                value: action().wait_before_use_millis_random_range,
            }
            div {} // Spacer

            // Wait after use
            ActionsMillisInput {
                label: "Wait after",
                on_value: move |millis| {
                    let mut action = action.write();
                    action.wait_after_use_millis = millis;
                },
                value: action().wait_after_use_millis,
            }
            ActionsMillisInput {
                label: "Wait random range",
                on_value: move |millis| {
                    let mut action = action.write();
                    action.wait_after_use_millis_random_range = millis;
                },
                value: action().wait_after_use_millis_random_range,
            }
        }
        div { class: "flex w-full gap-3 absolute bottom-0 py-2 bg-gray-900",
            Button {
                class: "flex-grow h-6 border border-gray-600",
                text: if modifying { "Save" } else { "Add" },
                kind: ButtonKind::Primary,
                on_click: move |_| {
                    on_value((*action.peek(), value.condition));
                },
            }
            Button {
                class: "flex-grow h-6 border border-gray-600",
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
fn ActionList(
    on_add_click: EventHandler,
    on_item_click: EventHandler<(Action, usize)>,
    on_item_move: EventHandler<(usize, ActionCondition, bool)>,
    on_item_delete: EventHandler<usize>,
    condition_filter: ActionCondition,
    disabled: bool,
    actions: Vec<Action>,
) -> Element {
    #[component]
    fn Icons(
        condition_filter: ActionCondition,
        action: Action,
        index: usize,
        on_item_move: EventHandler<(usize, ActionCondition, bool)>,
        on_item_delete: EventHandler<usize>,
    ) -> Element {
        const ICON_CONTAINER_CLASS: &str = "w-4 h-4 flex justify-center items-center";
        const ICON_CLASS: &str = "w-[11px] h-[11px] fill-current";

        let container_margin = if matches!(action.condition(), ActionCondition::Linked) {
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
                        on_item_move((index, condition_filter, true));
                    },
                    UpArrowIcon { class: "{ICON_CLASS} text-gray-50" }
                }
                div {
                    class: ICON_CONTAINER_CLASS,
                    onclick: move |e| {
                        e.stop_propagation();
                        on_item_move((index, condition_filter, false));
                    },
                    DownArrowIcon { class: "{ICON_CLASS} text-gray-50" }
                }
                div {
                    class: ICON_CONTAINER_CLASS,
                    onclick: move |e| {
                        e.stop_propagation();
                        on_item_delete(index);
                    },
                    XIcon { class: "{ICON_CLASS} text-red-500" }
                }
            }
        }
    }

    let filtered = filter_actions(actions, condition_filter);

    rsx! {
        div { class: "flex flex-col",
            for (action , index) in filtered {
                div {
                    class: "relative group",
                    onclick: move |e| {
                        e.stop_propagation();
                        on_item_click((action, index));
                    },
                    match action {
                        Action::Move(action) => rsx! {
                            ActionMoveItem { action }
                        },
                        Action::Key(action) => rsx! {
                            ActionKeyItem { action }
                        },
                    }
                    Icons {
                        condition_filter,
                        action,
                        index,
                        on_item_move,
                        on_item_delete,
                    }
                }
            }
            Button {
                text: "Add action",
                kind: ButtonKind::Secondary,
                on_click: move |_| {
                    on_add_click(());
                },
                disabled,
                class: "w-full h-5 label mt-2",
            }
        }
    }
}

#[component]
fn ActionMoveItem(action: ActionMove) -> Element {
    let ActionMove {
        position:
            Position {
                x,
                x_random_range,
                y,
                allow_adjusting,
            },
        condition,
        wait_after_move_millis,
    } = action;

    let x_min = (x - x_random_range).max(0);
    let x_max = (x + x_random_range).max(0);
    let x = if x_min == x_max {
        format!("{x}")
    } else {
        format!("{x_min}~{x_max}")
    };
    let allow_adjusting = if allow_adjusting { " / Adjust" } else { "" };

    let position = format!("{x}, {y}{allow_adjusting}");
    let linked_action = if matches!(condition, ActionCondition::Linked) {
        ""
    } else {
        "mt-2"
    };

    rsx! {
        div { class: "grid grid-cols-[160px_100px_auto] h-4 label group-hover:bg-gray-900 {linked_action}",
            div { class: "{ACTION_ITEM_BORDER_CLASS} {ACTION_ITEM_TEXT_CLASS}", "{position}" }
            div { class: "{ACTION_ITEM_TEXT_CLASS}", "⏱︎ {wait_after_move_millis}ms" }
            div {}
        }
    }
}

#[component]
fn ActionKeyItem(action: ActionKey) -> Element {
    let ActionKey {
        key,
        link_key,
        count,
        position,
        condition,
        direction,
        with,
        queue_to_front,
        ..
    } = action;

    let position = if let Some(Position {
        x,
        y,
        x_random_range,
        allow_adjusting,
    }) = position
    {
        let x_min = (x - x_random_range).max(0);
        let x_max = (x + x_random_range).max(0);
        let x = if x_min == x_max {
            format!("{x}")
        } else {
            format!("{x_min}~{x_max}")
        };
        let allow_adjusting = if allow_adjusting { " / Adjust" } else { "" };

        format!("{x}, {y}{allow_adjusting}")
    } else {
        "ㄨ".to_string()
    };
    let queue_to_front = if queue_to_front.unwrap_or_default() {
        "⇈ / "
    } else {
        ""
    };
    let linked_action = if matches!(condition, ActionCondition::Linked) {
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

    rsx! {
        div { class: "grid grid-cols-[160px_100px_30px_auto] h-4 label group-hover:bg-gray-900 {linked_action}",
            div { class: "{ACTION_ITEM_BORDER_CLASS} {ACTION_ITEM_TEXT_CLASS}",
                "{queue_to_front}{position}"
            }
            div { class: "{ACTION_ITEM_BORDER_CLASS} {ACTION_ITEM_TEXT_CLASS}",
                "{link_key}{key} × {count}"
            }
            div { class: "{ACTION_ITEM_BORDER_CLASS} {ACTION_ITEM_TEXT_CLASS}",
                match direction {
                    ActionKeyDirection::Any => "⇆",
                    ActionKeyDirection::Left => "←",
                    ActionKeyDirection::Right => "→",
                }
            }
            div { class: "pr-10 {ACTION_ITEM_TEXT_CLASS}",
                match with {
                    ActionKeyWith::Any => "Any",
                    ActionKeyWith::Stationary => "Stationary",
                    ActionKeyWith::DoubleJump => "Double jump",
                }
            }
        }
    }
}

#[component]
fn ActionsSelect<T: 'static + Clone + PartialEq + Display + IntoEnumIterator>(
    label: &'static str,
    disabled: bool,
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
fn ActionsNumberInputI32(
    label: &'static str,
    #[props(default = false)] disabled: bool,
    on_value: EventHandler<i32>,
    value: i32,
) -> Element {
    rsx! {
        NumberInputI32 {
            label,
            input_class: INPUT_CLASS,
            disabled,
            on_value,
            value,
        }
    }
}

#[component]
fn ActionsNumberInputU32(
    label: &'static str,
    #[props(default = false)] disabled: bool,
    on_value: EventHandler<u32>,
    value: u32,
) -> Element {
    rsx! {
        NumberInputU32 {
            label,
            input_class: INPUT_CLASS,
            minimum_value: 1,
            disabled,
            on_value,
            value,
        }
    }
}

#[component]
fn ActionsMillisInput(label: &'static str, on_value: EventHandler<u64>, value: u64) -> Element {
    rsx! {
        MillisInput {
            label,
            input_class: INPUT_CLASS,
            on_value,
            value,
        }
    }
}

#[component]
fn ActionsCheckbox(
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

#[component]
fn ActionsKeyBindingInput(
    label: &'static str,
    disabled: bool,
    on_value: EventHandler<Option<KeyBinding>>,
    value: Option<KeyBinding>,
) -> Element {
    rsx! {
        KeyBindingInput {
            label,
            input_class: "h-6 border border-gray-600",
            disabled,
            optional: false,
            on_value: move |value: Option<KeyBinding>| {
                on_value(value);
            },
            value,
        }
    }
}

/// Finds the linked action index range where `action_index` is a non-linked action.
fn find_linked_action_range(actions: &[Action], action_index: usize) -> Option<Range<usize>> {
    if action_index + 1 >= actions.len() {
        return None;
    }
    let start = action_index + 1;
    if !matches!(actions[start].condition(), ActionCondition::Linked) {
        return None;
    }

    let mut end = start + 1;
    while end < actions.len() {
        if !matches!(actions[end].condition(), ActionCondition::Linked) {
            break;
        }
        end += 1;
    }

    Some(start..end)
}

/// Finds the last linked action index of the last action matching `condition_filter`.
fn find_last_linked_action_index(
    actions: &[Action],
    condition_filter: ActionCondition,
) -> Option<usize> {
    let condition_filter = discriminant(&condition_filter);
    let (mut last_index, _) = actions
        .iter()
        .enumerate()
        .rev()
        .find(|(_, action)| condition_filter == discriminant(&action.condition()))?;

    if let Some(range) = find_linked_action_range(actions, last_index) {
        last_index += range.count();
    }

    Some(last_index)
}

/// Filters `actions` to find action with condition matching `condition_filter` including linked
/// action(s) of that matching action.
///
/// Returns a [`Vec<(Action, usize)>`] where [`usize`] is the index of the action inside the
/// original `actions`.
fn filter_actions(actions: Vec<Action>, condition_filter: ActionCondition) -> Vec<(Action, usize)> {
    let condition_filter = discriminant(&condition_filter);
    let mut filtered = Vec::with_capacity(actions.len());
    let mut i = 0;
    while i < actions.len() {
        let action = actions[i];
        if condition_filter != discriminant(&action.condition()) {
            i += 1;
            continue;
        }

        filtered.push((action, i));
        if let Some(range) = find_linked_action_range(&actions, i) {
            filtered.extend(actions[range.clone()].iter().copied().zip(range.clone()));
            i += range.count();
        }
        i += 1;
    }

    filtered
}
