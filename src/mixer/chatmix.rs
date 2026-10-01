//! ChatMix: balance between the Game and Chat channels.
//!
//! The headset dial (or a software slider) yields a [`ChatMix`] pair of Game and Chat levels.
//! Headsets disagree on the convention: some report both at 100 when centred, others report
//! two values that sum to 100. Dividing by the larger value covers both: the centred dial
//! always means "both at full volume", and turning it lowers only the other side.

use crate::devices::settings::ChatMix;

/// Gain factors (0.0-1.0) applied to the Game and Chat channel volumes.
pub fn chatmix_factors(mix: ChatMix) -> (f32, f32) {
    let game = f32::from(mix.game.min(100));
    let chat = f32::from(mix.chat.min(100));
    let max = game.max(chat);
    if max == 0.0 {
        return (1.0, 1.0);
    }
    (game / max, chat / max)
}

/// ChatMix from a software slider: `-1.0` = all Game, `0.0` = balanced, `1.0` = all Chat.
pub fn chatmix_from_balance(balance: f32) -> ChatMix {
    let balance = if balance.is_finite() {
        balance.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let to_level = |factor: f32| (factor * 100.0).round().clamp(0.0, 100.0) as u8;
    ChatMix {
        game: to_level(if balance > 0.0 { 1.0 - balance } else { 1.0 }),
        chat: to_level(if balance < 0.0 { 1.0 + balance } else { 1.0 }),
    }
}

/// `volume` (0-100) scaled by `factor`, rounded to a whole percent.
pub(crate) fn scale_volume(volume: u8, factor: f32) -> u8 {
    (f32::from(volume.min(100)) * factor.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 100.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mix(game: u8, chat: u8) -> ChatMix {
        ChatMix { game, chat }
    }

    #[test]
    fn centred_dial_is_full_volume_in_both_conventions() {
        assert_eq!(chatmix_factors(mix(100, 100)), (1.0, 1.0));
        assert_eq!(chatmix_factors(mix(50, 50)), (1.0, 1.0));
    }

    #[test]
    fn turning_the_dial_lowers_only_the_other_side() {
        assert_eq!(chatmix_factors(mix(100, 40)), (1.0, 0.4));
        assert_eq!(chatmix_factors(mix(25, 100)), (0.25, 1.0));
        // Sum-to-100 convention, dial three quarters towards Game.
        let (g, c) = chatmix_factors(mix(75, 25));
        assert_eq!(g, 1.0);
        assert!((c - 1.0 / 3.0).abs() < 1e-6);
        assert_eq!(chatmix_factors(mix(100, 0)), (1.0, 0.0));
    }

    #[test]
    fn degenerate_values_are_safe() {
        assert_eq!(chatmix_factors(mix(0, 0)), (1.0, 1.0));
        assert_eq!(chatmix_factors(mix(255, 100)), (1.0, 1.0));
    }

    #[test]
    fn balance_slider_maps_to_levels() {
        assert_eq!(chatmix_from_balance(0.0), mix(100, 100));
        assert_eq!(chatmix_from_balance(-1.0), mix(100, 0));
        assert_eq!(chatmix_from_balance(1.0), mix(0, 100));
        assert_eq!(chatmix_from_balance(0.5), mix(50, 100));
        assert_eq!(chatmix_from_balance(-0.25), mix(100, 75));
        assert_eq!(chatmix_from_balance(f32::NAN), mix(100, 100));
        assert_eq!(chatmix_from_balance(7.0), mix(0, 100));
    }

    #[test]
    fn effective_volume_rounds() {
        assert_eq!(scale_volume(80, 0.5), 40);
        assert_eq!(scale_volume(75, 1.0 / 3.0), 25);
        assert_eq!(scale_volume(100, 1.0), 100);
        assert_eq!(scale_volume(200, 1.0), 100);
    }
}
