//! The game's one font: Fira Mono Medium in full (`assets/fonts/FiraMono-Medium.ttf`, SIL
//! Open Font License), put behind Bevy's default font handle. Bevy's own is the same face
//! cut down to ASCII; this one keeps the look and adds the rest of Latin (å ä ö, é, ü, ß,
//! dashes and curly quotes), so crawls, titles and taglines can be written in Swedish.
//! Every `TextFont` that names no font of its own uses it.

use bevy::asset::AssetId;
use bevy::prelude::*;

const FONT: &[u8] = include_bytes!("../../assets/fonts/FiraMono-Medium.ttf");

/// Add after `DefaultPlugins` (its `TextPlugin` puts the ASCII cut there first).
pub struct GameFontPlugin;

impl Plugin for GameFontPlugin {
    fn build(&self, app: &mut App) {
        let mut fonts = app.world_mut().resource_mut::<Assets<Font>>();
        if let Err(e) = fonts.insert(AssetId::default(), Font::from_bytes(FONT.to_vec())) {
            error!("could not install the game font: {e}");
        }
    }
}

/// Whether the game font draws `c`: ASCII, Latin-1, Latin Extended-A and the common
/// typographic punctuation. Emoji, arrows and other scripts show as nothing.
pub fn drawable(c: char) -> bool {
    !c.is_control()
        && (c.is_ascii()
            || ('\u{a0}'..='\u{17f}').contains(&c)
            || "\u{2013}\u{2014}\u{2018}\u{2019}\u{201c}\u{201d}\u{2026}\u{20ac}".contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swedish_is_drawn_emoji_is_not() {
        assert!("Är det här Salems öde? – Ja, så klart…".chars().all(drawable));
        assert!(!drawable('\u{1f47d}'));
        assert!(!drawable('\u{2192}'));
        assert!(!drawable('\n'));
    }
}
