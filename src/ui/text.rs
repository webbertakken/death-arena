use bevy::prelude::*;
use std::ptr;

pub struct TextBox {
    pub font: Handle<Font>,
}

impl TextBox {
    pub fn button(&self, text: impl Into<String>, color: Color) -> TextBundle {
        TextBundle {
            text: Text::from_section(
                text,
                TextStyle {
                    font: self.font.clone(),
                    font_size: 32.0,
                    color,
                },
            ),
            ..default()
        }
    }

    pub fn title(&self, text: impl Into<String>, color: Color) -> TextBundle {
        TextBundle {
            style: Style {
                margin: UiRect::new(Val::Px(0.0), Val::Px(0.0), Val::Px(0.0), Val::Px(20.0)),
                ..default()
            },
            text: Text::from_section(
                text,
                TextStyle {
                    font: self.font.clone(),
                    font_size: 48.0,
                    color,
                },
            ),
            ..default()
        }
    }
}
