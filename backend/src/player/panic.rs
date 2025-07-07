use platforms::windows::KeyKind;

use super::{
    Player, PlayerState,
    actions::{PanicTo, on_action},
    timeout::Timeout,
};
use crate::{
    bridge::MouseAction,
    context::Context,
    minimap::Minimap,
    player::timeout::{Lifecycle, next_timeout_lifecycle},
};

const MAX_RETRY: u32 = 4;

/// Stages of panicking mode.
#[derive(Debug, Clone, Copy)]
enum PanickingStage {
    /// Cycling through channels.
    ChangingChannel(Timeout, u32),
    /// Going to town.
    GoingToTown(Timeout, u32),
    Completing(Timeout, bool),
}

#[derive(Debug, Clone, Copy)]
pub struct Panicking {
    stage: PanickingStage,
    pub to: PanicTo,
}

impl Panicking {
    pub fn new(to: PanicTo) -> Self {
        Self {
            stage: match to {
                PanicTo::Channel => PanickingStage::ChangingChannel(Timeout::default(), 0),
                PanicTo::Town => PanickingStage::GoingToTown(Timeout::default(), 0),
            },
            to,
        }
    }

    #[inline]
    fn stage_changing_channel(self, timeout: Timeout, retry_count: u32) -> Panicking {
        Panicking {
            stage: PanickingStage::ChangingChannel(timeout, retry_count),
            ..self
        }
    }

    #[inline]
    fn stage_going_to_town(self, timeout: Timeout, retry_count: u32) -> Panicking {
        Panicking {
            stage: PanickingStage::GoingToTown(timeout, retry_count),
            ..self
        }
    }

    #[inline]
    fn stage_completing(self, timeout: Timeout, completed: bool) -> Panicking {
        Panicking {
            stage: PanickingStage::Completing(timeout, completed),
            ..self
        }
    }
}

/// Updates [`Player::Panicking`] contextual state.
pub fn update_panicking_context(
    context: &Context,
    state: &mut PlayerState,
    panicking: Panicking,
) -> Player {
    let panicking = match panicking.stage {
        PanickingStage::ChangingChannel(timeout, retry_count) => update_changing_channel(
            context,
            state.config.change_channel_key,
            panicking,
            timeout,
            retry_count,
        ),
        PanickingStage::GoingToTown(timeout, retry_count) => update_going_to_town(
            context,
            state.config.maple_guide_key,
            panicking,
            timeout,
            retry_count,
        ),
        PanickingStage::Completing(timeout, completed) => {
            update_completing(context, panicking, timeout, completed)
        }
    };
    let next = if matches!(panicking.stage, PanickingStage::Completing(_, true)) {
        Player::Idle
    } else {
        Player::Panicking(panicking)
    };

    on_action(
        state,
        |_| Some((next, matches!(next, Player::Idle))),
        || Player::Idle, // Force cancel if it is not initiated from an action
    )
}

fn update_changing_channel(
    context: &Context,
    key: KeyKind,
    panicking: Panicking,
    timeout: Timeout,
    retry_count: u32,
) -> Panicking {
    const PRESS_RIGHT_AT_AFTER: u32 = 15;
    const PRESS_ENTER_AT_AFTER: u32 = 30;
    const TIMEOUT_AFTER: u32 = 50;

    const TIMEOUT_INITIAL: u32 = 220;
    const PRESS_RIGHT_AT_INITIAL: u32 = 170;
    const PRESS_ENTER_AT_INITIAL: u32 = 200;

    let max_timeout = if retry_count == 0 {
        TIMEOUT_INITIAL
    } else {
        TIMEOUT_AFTER
    };
    match next_timeout_lifecycle(timeout, max_timeout) {
        Lifecycle::Started(timeout) => {
            if !context
                .detector_unwrap()
                .detect_change_channel_menu_opened()
            {
                let _ = context.keys.send(key);
            }

            panicking.stage_changing_channel(timeout, retry_count)
        }
        Lifecycle::Ended => {
            if matches!(context.minimap, Minimap::Idle(_)) {
                if retry_count + 1 < MAX_RETRY {
                    panicking.stage_changing_channel(Timeout::default(), retry_count + 1)
                } else {
                    panicking.stage_completing(Timeout::default(), true)
                }
            } else {
                panicking.stage_completing(Timeout::default(), false)
            }
        }
        Lifecycle::Updated(timeout) => {
            let (press_right_at, press_enter_at) = if retry_count == 0 {
                (PRESS_RIGHT_AT_INITIAL, PRESS_ENTER_AT_INITIAL)
            } else {
                (PRESS_RIGHT_AT_AFTER, PRESS_ENTER_AT_AFTER)
            };
            match timeout.current {
                tick if tick == press_right_at => {
                    if context
                        .detector_unwrap()
                        .detect_change_channel_menu_opened()
                    {
                        let _ = context.keys.send(KeyKind::Right);
                    }
                }
                tick if tick == press_enter_at => {
                    if context
                        .detector_unwrap()
                        .detect_change_channel_menu_opened()
                    {
                        let _ = context.keys.send(KeyKind::Enter);
                    }
                }
                _ => (),
            }

            panicking.stage_changing_channel(timeout, retry_count)
        }
    }
}

fn update_going_to_town(
    context: &Context,
    key: KeyKind,
    panicking: Panicking,
    timeout: Timeout,
    retry_count: u32,
) -> Panicking {
    const GUIDE_FULLY_OPENED_CHECK_AT: u32 = 30;

    match next_timeout_lifecycle(timeout, 50) {
        Lifecycle::Started(timeout) => {
            if matches!(context.minimap, Minimap::Idle(_)) {
                if !context.detector_unwrap().detect_maple_guide_menu_opened() {
                    let _ = context.keys.send(key);
                }
            } else {
                return panicking.stage_completing(Timeout::default(), false);
            }

            panicking.stage_going_to_town(timeout, retry_count)
        }
        Lifecycle::Ended => {
            if context.detector_unwrap().detect_maple_guide_menu_opened() {
                let towns = context.detector_unwrap().detect_maple_guide_towns();
                let town = context.rng.random_choose(&towns);
                if let Some(town) = town {
                    let x = town.x + town.width / 2;
                    let y = town.y + town.height / 2;
                    let _ = context.keys.send_mouse(x, y, MouseAction::Click);
                }
            }

            if retry_count + 1 < MAX_RETRY {
                panicking.stage_going_to_town(Timeout::default(), retry_count + 1)
            } else {
                panicking.stage_completing(Timeout::default(), true)
            }
        }
        Lifecycle::Updated(timeout) => {
            if timeout.current == GUIDE_FULLY_OPENED_CHECK_AT
                && !context.detector_unwrap().detect_maple_guide_menu_opened()
            {
                let _ = context.keys.send(key);
            }

            panicking.stage_going_to_town(timeout, retry_count)
        }
    }
}

fn update_completing(
    context: &Context,
    panicking: Panicking,
    timeout: Timeout,
    completed: bool,
) -> Panicking {
    if matches!(panicking.to, PanicTo::Town) {
        return panicking.stage_completing(timeout, true);
    }

    match next_timeout_lifecycle(timeout, 245) {
        Lifecycle::Ended => {
            if let Minimap::Idle(idle) = context.minimap {
                if idle.has_any_other_player() {
                    panicking.stage_changing_channel(Timeout::default(), 0)
                } else {
                    panicking.stage_completing(timeout, true)
                }
            } else {
                panicking.stage_completing(Timeout::default(), false)
            }
        }
        Lifecycle::Started(timeout) | Lifecycle::Updated(timeout) => {
            panicking.stage_completing(timeout, completed)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches::assert_matches;

    use anyhow::Ok;

    use super::*;
    use crate::{
        bridge::MockKeySender,
        detect::MockDetector,
        minimap::{Minimap, MinimapIdle},
    };

    #[test]
    fn update_changing_channel_and_send_keys() {
        let mut keys = MockKeySender::default();
        let mut detector = MockDetector::default();
        detector
            .expect_detect_change_channel_menu_opened()
            .return_const(true);
        keys.expect_send().times(2).returning(|_| Ok(()));
        let context = Context::new(Some(keys), Some(detector));
        let panicking = Panicking::new(PanicTo::Channel);

        let timeout = Timeout {
            current: 169,
            started: true,
            ..Default::default()
        };
        let result = update_changing_channel(&context, KeyKind::F1, panicking, timeout, 0);
        assert_matches!(result.stage, PanickingStage::ChangingChannel(_, _));

        let timeout = Timeout {
            current: 199,
            started: true,
            ..Default::default()
        };
        let result = update_changing_channel(&context, KeyKind::F1, panicking, timeout, 0);
        assert_matches!(result.stage, PanickingStage::ChangingChannel(_, _));
    }

    #[test]
    fn update_changing_channel_and_send_keys_retry() {
        let mut keys = MockKeySender::default();
        let mut detector = MockDetector::default();
        detector
            .expect_detect_change_channel_menu_opened()
            .return_const(true);
        keys.expect_send().times(2).returning(|_| Ok(()));
        let context = Context::new(Some(keys), Some(detector));
        let panicking = Panicking::new(PanicTo::Channel);

        let timeout = Timeout {
            current: 14,
            started: true,
            ..Default::default()
        };
        let result = update_changing_channel(&context, KeyKind::F1, panicking, timeout, 1);
        assert_matches!(result.stage, PanickingStage::ChangingChannel(_, _));

        let timeout = Timeout {
            current: 29,
            started: true,
            ..Default::default()
        };
        let result = update_changing_channel(&context, KeyKind::F1, panicking, timeout, 1);
        assert_matches!(result.stage, PanickingStage::ChangingChannel(_, _));
    }

    #[test]
    fn update_changing_channel_complete_if_minimap_not_idle() {
        let mut context = Context::new(None, None);
        context.minimap = Minimap::Detecting;
        let panicking = Panicking::new(PanicTo::Channel);
        let timeout = Timeout {
            current: 220,
            started: true,
            ..Default::default()
        };

        let result = update_changing_channel(&context, KeyKind::F1, panicking, timeout, 0);
        assert_matches!(result.stage, PanickingStage::Completing(_, false));
    }

    #[test]
    fn update_changing_channel_complete_if_minimap_not_idle_retry() {
        let mut context = Context::new(None, None);
        context.minimap = Minimap::Detecting;
        let panicking = Panicking::new(PanicTo::Channel);
        let timeout = Timeout {
            current: 50,
            started: true,
            ..Default::default()
        };

        let result = update_changing_channel(&context, KeyKind::F1, panicking, timeout, 1);
        assert_matches!(result.stage, PanickingStage::Completing(_, false));
    }

    #[test]
    fn update_going_to_town_send_key_if_menu_not_open_and_minimap_idle() {
        let mut keys = MockKeySender::default();
        keys.expect_send().once().returning(|_| Ok(()));
        let mut detector = MockDetector::default();
        detector
            .expect_detect_maple_guide_menu_opened()
            .return_const(false);
        let mut context = Context::new(Some(keys), Some(detector));
        context.minimap = Minimap::Idle(MinimapIdle::default());

        let panicking = Panicking::new(PanicTo::Town);
        let timeout = Timeout::default();

        let result = update_going_to_town(&context, KeyKind::F2, panicking, timeout, 0);
        assert_matches!(result.stage, PanickingStage::GoingToTown(_, _));
    }

    #[test]
    fn update_going_to_town_complete_if_not_idle_minimap() {
        let mut detector = MockDetector::default();
        detector
            .expect_detect_maple_guide_menu_opened()
            .return_const(true);
        let context = Context::new(None, Some(detector));

        let panicking = Panicking::new(PanicTo::Town);
        let timeout = Timeout::default();

        let result = update_going_to_town(&context, KeyKind::F2, panicking, timeout, 0);
        assert_matches!(result.stage, PanickingStage::Completing(_, false));
    }

    #[test]
    fn update_completing_for_town_immediately_complete() {
        let context = Context::new(None, None);
        let panicking = Panicking::new(PanicTo::Town);

        let timeout = Timeout::default();
        let result = update_completing(&context, panicking, timeout, false);
        assert_matches!(result.stage, PanickingStage::Completing(_, true));
    }

    #[test]
    fn update_completing_for_channel_switch_to_idle_if_no_players() {
        let mut context = Context::new(None, None);
        context.minimap = Minimap::Idle(MinimapIdle::default());
        let panicking = Panicking::new(PanicTo::Channel);
        let timeout = Timeout {
            current: 245,
            started: true,
            ..Default::default()
        };

        let result = update_completing(&context, panicking, timeout, false);
        assert_matches!(result.stage, PanickingStage::Completing(_, true));
    }
}
