use bevy::prelude::*;

pub struct TextBox {
    pub font: Handle<Font>,
}

impl TextBox {
    pub fn button(&self, text: impl Into<String>, color: Color) -> impl Bundle {
        (
            Text::new(text),
            TextFont {
                font: self.font.clone(),
                font_size: 32.0,
                ..default()
            },
            TextColor(color),
        )
    }

    pub fn title(&self, text: impl Into<String>, color: Color) -> impl Bundle {
        (
            Text::new(text),
            TextFont {
                font: self.font.clone(),
                font_size: 48.0,
                ..default()
            },
            TextColor(color),
            Node {
                margin: UiRect::new(Val::Px(0.0), Val::Px(0.0), Val::Px(0.0), Val::Px(20.0)),
                ..default()
            },
        )
    }
}
