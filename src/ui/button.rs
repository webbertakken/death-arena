use bevy::prelude::*;

pub struct Button {}

pub fn setup() -> Button {
    Button {}
}

impl Button {
    pub fn create(&self, color: Color) -> ButtonBundle {
        ButtonBundle {
            style: Style {
                display: Display::Flex,
                size: Size::new(Val::Percent(100.0), Val::Px(50.0)),
                margin: UiRect::all(Val::Px(0.0)),
                padding: UiRect::all(Val::Px(8.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..Default::default()
            },
            color: UiColor::from(color),
            transform: Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),
            ..Default::default()
        }
    }
}
