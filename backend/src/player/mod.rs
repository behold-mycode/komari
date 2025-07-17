use actions::{on_action, on_action_state_mut};
use adjust::{Adjusting, update_adjusting_context};
use cash_shop::{CashShop, update_cash_shop_context};
use double_jump::{DoubleJumping, update_double_jumping_context};
use fall::update_falling_context;
use familiars_swap::{FamiliarsSwapping, update_familiars_swapping_context};
use grapple::update_grappling_context;
use idle::update_idle_context;
use jump::update_jumping_context;
use moving::{MOVE_TIMEOUT, Moving, MovingIntermediates, update_moving_context};
use opencv::core::Point;
use panic::update_panicking_context;
#[cfg(windows)]
use platforms::windows::KeyKind;
#[cfg(target_os = "macos")]
use platforms::macos::KeyKind;
use solve_rune::{SolvingRune, update_solving_rune_context};
use stall::update_stalling_context;
use state::LastMovement;
use strum::Display;
use timeout::Timeout;
use unstuck::update_unstucking_context;
use up_jump::{UpJumping, update_up_jumping_context};
use use_key::{UseKey, update_use_key_context};

use crate::{
    context::{Context, Contextual, ControlFlow},
    database::ActionKeyDirection,
    minimap::Minimap,
};

mod actions;
mod adjust;
mod cash_shop;
mod double_jump;
mod fall;
mod familiars_swap;
mod grapple;
mod idle;
mod jump;
mod moving;
mod panic;
mod solve_rune;
mod stall;
mod state;
mod timeout;
mod unstuck;
mod up_jump;
mod use_key;

pub use {
    actions::PanicTo, actions::PingPongDirection, actions::PlayerAction,
    actions::PlayerActionAutoMob, actions::PlayerActionFamiliarsSwapping, actions::PlayerActionKey,
    actions::PlayerActionMove, actions::PlayerActionPanic, actions::PlayerActionPingPong,
    double_jump::DOUBLE_JUMP_THRESHOLD, grapple::GRAPPLING_MAX_THRESHOLD,
    grapple::GRAPPLING_THRESHOLD, panic::Panicking, state::PlayerState, state::Quadrant,
};

/// Minimum y distance from the destination required to perform a jump.
pub const JUMP_THRESHOLD: i32 = 7;

/// The player contextual states.
#[derive(Clone, Copy, Debug, Display)]
#[allow(clippy::large_enum_variant)] // There is only ever a single instance of Player
pub enum Player {
    /// Detects player on the minimap.
    Detecting,
    /// Does nothing state.
    ///
    /// Acts as entry to other state when there is a [`PlayerAction`].
    Idle,
    /// Uses key.
    UseKey(UseKey),
    /// Movement-related coordinator state.
    Moving(Point, bool, Option<MovingIntermediates>),
    /// Performs walk or small adjustment x-wise action.
    Adjusting(Adjusting),
    /// Performs double jump action.
    DoubleJumping(DoubleJumping),
    /// Performs a grappling action.
    Grappling(Moving),
    /// Performs a normal jump.
    Jumping(Moving),
    /// Performs an up jump action.
    UpJumping(UpJumping),
    /// Performs a falling action.
    Falling {
        moving: Moving,
        anchor: Point,
        timeout_on_complete: bool,
    },
    /// Unstucks when inside non-detecting position or because of [`PlayerState::unstuck_counter`].
    Unstucking(Timeout, Option<bool>, bool),
    /// Stalls for time and return to [`Player::Idle`] or [`PlayerState::stalling_timeout_state`].
    Stalling(Timeout, u32),
    /// Tries to solve a rune.
    SolvingRune(SolvingRune),
    /// Enters the cash shop then exit after 10 seconds.
    CashShopThenExit(Timeout, CashShop),
    #[strum(to_string = "FamiliarsSwapping({0})")]
    FamiliarsSwapping(FamiliarsSwapping),
    Panicking(Panicking),
}

impl Player {
    #[inline]
    pub fn can_action_override_current_state(&self, cur_pos: Option<Point>) -> bool {
        const OVERRIDABLE_DISTANCE: i32 = DOUBLE_JUMP_THRESHOLD / 2;

        match self {
            Player::Detecting | Player::Idle => true,
            Player::Moving(dest, _, _) => {
                if let Some(pos) = cur_pos {
                    (dest.x - pos.x).abs() >= OVERRIDABLE_DISTANCE
                } else {
                    true
                }
            }
            Player::DoubleJumping(DoubleJumping {
                moving,
                forced: false,
                ..
            })
            | Player::Adjusting(Adjusting { moving, .. }) => {
                let (distance, _) =
                    moving.x_distance_direction_from(true, cur_pos.unwrap_or(moving.pos));
                distance >= OVERRIDABLE_DISTANCE
            }
            Player::Grappling(moving)
            | Player::Jumping(moving)
            | Player::UpJumping(UpJumping { moving, .. })
            | Player::Falling {
                moving,
                anchor: _,
                timeout_on_complete: _,
            } => moving.completed,
            Player::SolvingRune(_)
            | Player::CashShopThenExit(_, _)
            | Player::Unstucking(_, _, _)
            | Player::DoubleJumping(DoubleJumping { forced: true, .. })
            | Player::UseKey(_)
            | Player::FamiliarsSwapping(_)
            | Player::Panicking(_)
            | Player::Stalling(_, _) => false,
        }
    }
}

impl Contextual for Player {
    type Persistent = PlayerState;

    // TODO: Detect if a point is reachable after number of retries?
    fn update(self, context: &Context, state: &mut PlayerState) -> ControlFlow<Self> {
        if state.rune_cash_shop {
            let _ = context.keys.send_up(KeyKind::Up);
            let _ = context.keys.send_up(KeyKind::Down);
            let _ = context.keys.send_up(KeyKind::Left);
            let _ = context.keys.send_up(KeyKind::Right);
            state.rune_cash_shop = false;
            state.reset_to_idle_next_update = false;
            return ControlFlow::Next(Player::CashShopThenExit(
                Timeout::default(),
                CashShop::Entering,
            ));
        }

        let has_position = if state.ignore_pos_update {
            state.last_known_pos.is_some()
        } else {
            state
                .update_state(context)
                .then(|| state.last_known_pos.unwrap())
                .is_some()
        };
        if !has_position {
            // When the player detection fails, the possible causes are:
            // - Player moved inside the edges of the minimap
            // - Other UIs overlapping the minimap
            //
            // `update_non_positional_context` is here to continue updating
            // `Player::Unstucking` returned from below when the player
            // is inside the edges of the minimap. And also `Player::CashShopThenExit`.
            if let Some(next) = update_non_positional_context(self, context, state, true) {
                return ControlFlow::Next(next);
            }
            let next = if !context.halting
                && let Minimap::Idle(idle) = context.minimap
                && !idle.partially_overlapping
            {
                Player::Unstucking(
                    Timeout::default(),
                    None,
                    state.track_unstucking_transitioned(),
                )
            } else {
                Player::Detecting
            };
            if matches!(next, Player::Unstucking(_, _, _)) {
                state.last_known_direction = ActionKeyDirection::Any;
            }
            return ControlFlow::Next(next);
        };

        let contextual = if state.reset_to_idle_next_update {
            Player::Idle
        } else {
            self
        };
        let next = update_non_positional_context(contextual, context, state, false)
            .unwrap_or_else(|| update_positional_context(contextual, context, state));
        let control_flow = if state.use_immediate_control_flow {
            ControlFlow::Immediate(next)
        } else {
            ControlFlow::Next(next)
        };

        state.reset_to_idle_next_update = false;
        state.ignore_pos_update = state.use_immediate_control_flow;
        state.use_immediate_control_flow = false;
        control_flow
    }
}

/// Updates the contextual state that does not require the player current position
#[inline]
fn update_non_positional_context(
    contextual: Player,
    context: &Context,
    state: &mut PlayerState,
    failed_to_detect_player: bool,
) -> Option<Player> {
    match contextual {
        Player::UseKey(use_key) => {
            (!failed_to_detect_player).then(|| update_use_key_context(context, state, use_key))
        }
        Player::FamiliarsSwapping(swapping) => {
            Some(update_familiars_swapping_context(context, state, swapping))
        }
        Player::Unstucking(timeout, has_settings, gamba_mode) => Some(update_unstucking_context(
            context,
            state,
            timeout,
            has_settings,
            gamba_mode,
        )),
        Player::Stalling(timeout, max_timeout) => {
            (!failed_to_detect_player).then(|| update_stalling_context(state, timeout, max_timeout))
        }
        Player::SolvingRune(solving_rune) => (!failed_to_detect_player)
            .then(|| update_solving_rune_context(context, state, solving_rune)),
        Player::CashShopThenExit(timeout, cash_shop) => Some(update_cash_shop_context(
            context,
            state,
            timeout,
            cash_shop,
            failed_to_detect_player,
        )),
        Player::Panicking(panicking) => Some(update_panicking_context(context, state, panicking)),
        Player::Detecting
        | Player::Idle
        | Player::Moving(_, _, _)
        | Player::Adjusting(_)
        | Player::DoubleJumping(_)
        | Player::Grappling(_)
        | Player::Jumping(_)
        | Player::UpJumping(_)
        | Player::Falling {
            moving: _,
            anchor: _,
            timeout_on_complete: _,
        } => None,
    }
}

/// Updates the contextual state that requires the player current position
#[inline]
fn update_positional_context(
    contextual: Player,
    context: &Context,
    state: &mut PlayerState,
) -> Player {
    match contextual {
        Player::Detecting => Player::Idle,
        Player::Idle => update_idle_context(context, state),
        Player::Moving(dest, exact, intermediates) => {
            update_moving_context(context, state, dest, exact, intermediates)
        }
        Player::Adjusting(adjusting) => update_adjusting_context(context, state, adjusting),
        Player::DoubleJumping(double_jumping) => {
            update_double_jumping_context(context, state, double_jumping)
        }
        Player::Grappling(moving) => update_grappling_context(context, state, moving),
        Player::UpJumping(moving) => update_up_jumping_context(context, state, moving),
        Player::Jumping(moving) => update_jumping_context(context, state, moving),
        Player::Falling {
            moving,
            anchor,
            timeout_on_complete,
        } => update_falling_context(context, state, moving, anchor, timeout_on_complete),
        Player::UseKey(_)
        | Player::Unstucking(_, _, _)
        | Player::Stalling(_, _)
        | Player::SolvingRune(_)
        | Player::FamiliarsSwapping(_)
        | Player::Panicking(_)
        | Player::CashShopThenExit(_, _) => unreachable!(),
    }
}
