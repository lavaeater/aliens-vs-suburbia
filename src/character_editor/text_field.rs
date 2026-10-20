//! A one-line text field: click it, type, Enter or Escape to finish.
//!
//! While a field has focus it owns the keyboard: `focus::EditorInputFocus::text_capture`
//! is set, which stops the character walking on the letters typed. Up / Down / Enter are
//! not typed into the field but reported as [`FieldKey`] messages, so whatever the field
//! filters (the Library's list) can be stepped through and opened without the mouse.
//!
//! Clicking anywhere else, or Escape, gives the keyboard back.
//!
//! Text is ASCII only, like everything else on screen: Bevy's default font has nothing
//! more, and a character it cannot draw would sit in the value invisibly.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::picking::hover::HoverMap;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use crate::character_editor::focus::EditorInputFocus;

/// A text field's value. Spawn with [`text_field`].
#[derive(Component, Debug, Default, Clone)]
pub struct TextField {
    pub value: String,
    pub placeholder: &'static str,
}

/// The text node inside a field, redrawn from the field's value.
#[derive(Component)]
pub struct TextFieldLabel;

/// The field with the keyboard, if any.
#[derive(Resource, Debug, Default)]
pub struct FocusedField(pub Option<Entity>);

/// Keys a focused field passes on rather than typing.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKey {
    Up(Entity),
    Down(Entity),
    Submit(Entity),
}

const IDLE_BG: Color = Color::srgba(0.07, 0.10, 0.14, 1.0);
const FOCUSED_BG: Color = Color::srgba(0.10, 0.18, 0.24, 1.0);

/// A field ready to spawn. Add a marker of your own to find it again.
pub fn text_field(placeholder: &'static str) -> impl Bundle {
    text_field_with(String::new(), placeholder)
}

/// A field that starts with `value` in it.
pub fn text_field_with(value: String, placeholder: &'static str) -> impl Bundle {
    let shown = if value.is_empty() {
        placeholder.to_string()
    } else {
        value.clone()
    };
    (
        TextField { value, placeholder },
        Node {
            padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
            align_self: AlignSelf::Stretch,
            flex_shrink: 0.0,
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..Default::default()
        },
        BackgroundColor(IDLE_BG),
        bevy::picking::hover::Hovered::default(),
        bevy::ui_widgets::Button,
        children![(
            TextFieldLabel,
            Text::new(shown),
            TextFont::default().with_font_size(12.0),
            TextColor(Color::srgb(0.5, 0.55, 0.6)),
        )],
    )
}

/// What a key does to a field's value. `None` = not a typing key.
pub fn edit_value(value: &mut String, key: &Key) -> bool {
    match key {
        Key::Backspace => {
            value.pop();
            true
        }
        Key::Space => {
            value.push(' ');
            true
        }
        Key::Character(text) => {
            let typed: String = text.chars().filter(|c| c.is_ascii() && !c.is_control()).collect();
            value.push_str(&typed);
            !typed.is_empty()
        }
        _ => false,
    }
}

/// Clicking a field gives it the keyboard.
pub fn focus_on_click(activate: On<Activate>, fields: Query<(), With<TextField>>, mut focused: ResMut<FocusedField>) {
    if fields.contains(activate.entity) {
        focused.0 = Some(activate.entity);
    }
}

/// Clicking anywhere but the focused field takes the keyboard back.
pub fn blur_on_click_elsewhere(
    mouse: Res<ButtonInput<MouseButton>>,
    hover_map: Res<HoverMap>,
    parents: Query<&ChildOf>,
    mut focused: ResMut<FocusedField>,
) {
    let Some(field) = focused.0 else {
        return;
    };
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let on_field = hover_map.values().any(|hits| {
        hits.keys().any(|&entity| {
            entity == field || parents.iter_ancestors(entity).any(|a| a == field)
        })
    });
    if !on_field {
        focused.0 = None;
    }
}

/// Type into the focused field.
pub fn type_into_field(
    mut keys: MessageReader<KeyboardInput>,
    mut focused: ResMut<FocusedField>,
    mut fields: Query<&mut TextField>,
    mut field_keys: MessageWriter<FieldKey>,
) {
    let Some(entity) = focused.0 else {
        keys.clear();
        return;
    };
    let Ok(mut field) = fields.get_mut(entity) else {
        // The field went away with its screen.
        focused.0 = None;
        return;
    };
    for event in keys.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        match &event.logical_key {
            Key::Escape => {
                focused.0 = None;
                return;
            }
            Key::Enter => {
                field_keys.write(FieldKey::Submit(entity));
            }
            Key::ArrowUp => {
                field_keys.write(FieldKey::Up(entity));
            }
            Key::ArrowDown => {
                field_keys.write(FieldKey::Down(entity));
            }
            key => {
                let mut value = field.value.clone();
                if edit_value(&mut value, key) && value != field.value {
                    field.value = value;
                }
            }
        }
    }
}

/// Tell the rest of the editor whether a field has the keyboard.
pub fn sync_text_capture(focused: Res<FocusedField>, mut input: ResMut<EditorInputFocus>) {
    let capturing = focused.0.is_some();
    if input.text_capture != capturing {
        input.text_capture = capturing;
    }
}

/// Redraw fields whose value or focus changed.
pub fn draw_fields(
    focused: Res<FocusedField>,
    mut fields: Query<(Entity, Ref<TextField>, &Children, &mut BackgroundColor)>,
    mut labels: Query<(&mut Text, &mut TextColor), With<TextFieldLabel>>,
) {
    for (entity, field, children, mut bg) in &mut fields {
        if !field.is_changed() && !focused.is_changed() {
            continue;
        }
        let has_focus = focused.0 == Some(entity);
        let wanted_bg = if has_focus { FOCUSED_BG } else { IDLE_BG };
        if bg.0 != wanted_bg {
            bg.0 = wanted_bg;
        }
        let (shown, color) = if has_focus {
            (format!("{}_", field.value), Color::srgb(0.9, 0.95, 1.0))
        } else if field.value.is_empty() {
            (field.placeholder.to_string(), Color::srgb(0.5, 0.55, 0.6))
        } else {
            (field.value.clone(), Color::srgb(0.85, 0.9, 0.95))
        };
        for &child in children {
            if let Ok((mut text, mut text_color)) = labels.get_mut(child) {
                if text.0 != shown {
                    text.0.clone_from(&shown);
                }
                text_color.0 = color;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_appends_and_backspace_removes() {
        let mut value = String::new();
        assert!(edit_value(&mut value, &Key::Character("a".into())));
        assert!(edit_value(&mut value, &Key::Space));
        assert!(edit_value(&mut value, &Key::Character("b".into())));
        assert_eq!(value, "a b");
        assert!(edit_value(&mut value, &Key::Backspace));
        assert_eq!(value, "a ");
    }

    /// The default font draws ASCII only; anything else would be typed invisibly.
    #[test]
    fn non_ascii_is_not_typed() {
        let mut value = String::new();
        assert!(!edit_value(&mut value, &Key::Character("e\u{301}".into())) || value == "e");
        assert!(value.is_ascii());
    }

    #[test]
    fn navigation_keys_are_not_text() {
        let mut value = String::from("x");
        assert!(!edit_value(&mut value, &Key::ArrowUp));
        assert!(!edit_value(&mut value, &Key::Enter));
        assert_eq!(value, "x");
    }
}
