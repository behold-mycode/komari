use std::cmp::Ordering;

use platforms::windows::KeyKind;

use super::{
    PlayerAction, PlayerActionKey, PlayerState,
    moving::Moving,
    timeout::{Lifecycle, next_timeout_lifecycle},
    use_key::UseKey,
};
use crate::{
    ActionKeyDirection, ActionKeyWith,
    context::Context,
    player::{
        Player,
        actions::{on_action_state, on_auto_mob_use_key_action},
        double_jump::DoubleJumping,
        moving::MOVE_TIMEOUT,
        state::LastMovement,
        timeout::{ChangeAxis, MovingLifecycle, Timeout, next_moving_lifecycle_with_axis},
    },
};

/// Minimum x distance from the destination required to perform small movement.
pub const ADJUSTING_SHORT_THRESHOLD: i32 = 1;

/// Minimum x distance from the destination required to walk.
pub const ADJUSTING_MEDIUM_THRESHOLD: i32 = 3;

const ADJUSTING_SHORT_TIMEOUT: u32 = 3;

/// Minimium y distance required to perform a fall and then walk.
const FALLING_THRESHOLD: i32 = 8;

#[derive(Clone, Copy, Debug)]
pub struct Adjusting {
    pub moving: Moving,
    adjust_timeout: Timeout,
}

impl Adjusting {
    pub fn new(moving: Moving) -> Self {
        Self {
            moving,
            adjust_timeout: Timeout::default(),
        }
    }

    fn moving(self, moving: Moving) -> Adjusting {
        Adjusting { moving, ..self }
    }

    fn update_adjusting(&mut self, context: &Context, up_key: KeyKind, down_key: KeyKind) {
        self.adjust_timeout =
            match next_timeout_lifecycle(self.adjust_timeout, ADJUSTING_SHORT_TIMEOUT) {
                Lifecycle::Started(timeout) => {
                    let _ = context.keys.send_up(up_key);
                    let _ = context.keys.send(down_key);
                    timeout
                }
                Lifecycle::Ended => Timeout::default(),
                Lifecycle::Updated(timeout) => timeout,
            };
    }
}

/// Updates the [`Player::Adjusting`] contextual state.
///
/// This state just walks towards the destination. If [`Moving::exact`] is true,
/// then it will perform small movement to ensure the `x` is as close as possible.
pub fn update_adjusting_context(
    context: &Context,
    state: &mut PlayerState,
    adjusting: Adjusting,
) -> Player {
    let moving = adjusting.moving;
    let cur_pos = state.last_known_pos.expect("in positional context");
    let (x_distance, x_direction) = moving.x_distance_direction_from(true, cur_pos);
    let is_intermediate = moving.is_destination_intermediate();

    match next_moving_lifecycle_with_axis(moving, cur_pos, MOVE_TIMEOUT, ChangeAxis::Both) {
        MovingLifecycle::Started(moving) => {
            // Check to perform a fall and returns to walk
            if !is_intermediate
                && state.last_movement != Some(LastMovement::Falling)
                && state.is_stationary
                && x_distance >= ADJUSTING_MEDIUM_THRESHOLD
            {
                let (y_distance, y_direction) = moving.y_distance_direction_from(true, cur_pos);
                if y_direction < 0 && y_distance >= FALLING_THRESHOLD {
                    return Player::Falling(moving.timeout_started(false), cur_pos, false);
                }
            }

            state.use_immediate_control_flow = true;
            state.last_movement = Some(LastMovement::Adjusting);

            Player::Adjusting(adjusting.moving(moving))
        }
        MovingLifecycle::Ended(moving) => {
            let _ = context.keys.send_up(KeyKind::Right);
            let _ = context.keys.send_up(KeyKind::Left);

            Player::Moving(moving.dest, moving.exact, moving.intermediates)
        }
        MovingLifecycle::Updated(mut moving) => {
            let mut adjusting = adjusting;

            if x_distance >= state.double_jump_threshold(is_intermediate) {
                state.use_immediate_control_flow = true;
                return Player::Moving(moving.dest, moving.exact, moving.intermediates);
            }

            if !moving.completed {
                let adjusting_started = adjusting.adjust_timeout.started;
                if adjusting_started {
                    // Do not allow timing out if adjusting is in-progress
                    moving = moving.timeout_current(moving.timeout.current.saturating_sub(1));
                }

                let should_adjust_medium =
                    !adjusting_started && x_distance >= ADJUSTING_MEDIUM_THRESHOLD;
                let should_adjust_short =
                    adjusting_started || (moving.exact && x_distance >= ADJUSTING_SHORT_THRESHOLD);
                let direction = match x_direction.cmp(&0) {
                    Ordering::Greater => {
                        Some((KeyKind::Right, KeyKind::Left, ActionKeyDirection::Right))
                    }
                    Ordering::Less => {
                        Some((KeyKind::Left, KeyKind::Right, ActionKeyDirection::Left))
                    }
                    _ => None,
                };

                match (should_adjust_medium, should_adjust_short, direction) {
                    (true, _, Some((down_key, up_key, dir))) => {
                        let _ = context.keys.send_up(up_key);
                        let _ = context.keys.send_down(down_key);
                        state.last_known_direction = dir;
                    }
                    (false, true, Some((down_key, up_key, dir))) => {
                        adjusting.update_adjusting(context, up_key, down_key);
                        state.last_known_direction = dir;
                    }
                    _ => {
                        let _ = context.keys.send_up(KeyKind::Left);
                        let _ = context.keys.send_up(KeyKind::Right);
                        moving = moving.completed(true);
                    }
                }
            }

            on_action_state(
                state,
                |state, action| on_player_action(context, state, action, moving),
                || {
                    if !moving.completed {
                        return Player::Adjusting(adjusting.moving(moving));
                    }

                    if moving.exact && x_distance >= ADJUSTING_SHORT_THRESHOLD {
                        // Exact adjusting incomplete
                        return Player::Adjusting(
                            adjusting.moving(moving.completed(false).timeout_current(0)),
                        );
                    }

                    Player::Adjusting(adjusting.moving(moving.timeout_current(MOVE_TIMEOUT)))
                },
            )
        }
    }
}

fn on_player_action(
    context: &Context,
    state: &PlayerState,
    action: PlayerAction,
    moving: Moving,
) -> Option<(Player, bool)> {
    const USE_KEY_Y_THRESHOLD: i32 = 2;

    let cur_pos = state.last_known_pos.unwrap();
    let (x_distance, _) = moving.x_distance_direction_from(false, cur_pos);
    let (y_distance, _) = moving.y_distance_direction_from(false, cur_pos);

    match action {
        PlayerAction::Key(PlayerActionKey {
            with: ActionKeyWith::DoubleJump,
            direction,
            ..
        }) => {
            if !moving.completed || y_distance > 0 {
                return None;
            }
            if matches!(direction, ActionKeyDirection::Any)
                || direction == state.last_known_direction
            {
                Some((
                    Player::DoubleJumping(DoubleJumping::new(
                        moving.timeout(Timeout::default()).completed(false),
                        true,
                        false,
                    )),
                    false,
                ))
            } else {
                Some((Player::UseKey(UseKey::from_action(action)), false))
            }
        }
        PlayerAction::Key(PlayerActionKey {
            with: ActionKeyWith::Any,
            ..
        }) => {
            if moving.completed && y_distance <= USE_KEY_Y_THRESHOLD {
                Some((Player::UseKey(UseKey::from_action(action)), false))
            } else {
                None
            }
        }
        PlayerAction::AutoMob(_) => {
            on_auto_mob_use_key_action(context, action, moving.pos, x_distance, y_distance)
        }
        PlayerAction::Key(PlayerActionKey {
            with: ActionKeyWith::Stationary,
            ..
        })
        | PlayerAction::SolveRune
        | PlayerAction::Move(_) => None,
        PlayerAction::PingPong(_) | PlayerAction::Panic(_) | PlayerAction::FamiliarsSwapping(_) => {
            unreachable!()
        }
    }
}
