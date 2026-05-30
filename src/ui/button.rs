use crate::ui::prelude::*;
use bevy::prelude::*;

#[derive(Default)]
pub struct UiButton {}

impl UiButton {
    pub fn normal(&self) -> ButtonBundle {
        ButtonBundle {
            style: Style {
                display: Display::Flex,
                size: Size::new(Val::Percent(100.0), Val::Px(50.0)),
                margin: UiRect::all(Val::Px(0.0)),
                padding: UiRect::all(Val::Px(8.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            background_color: BackgroundColor::from(BUTTON_COLOR),
            transform: Transform::from_translation(Vec3::ZERO),
            ..default()
        }
    }
}

pub fn styles_system(mut query: Query<(&Interaction, &mut BackgroundColor), With<Button>>) {
    for (interaction, mut color) in &mut query {
        match *interaction {
            Interaction::Clicked => {
                *color = BackgroundColor::from(BUTTON_ACTIVE_COLOR);
            }
            Interaction::Hovered => {
                *color = BackgroundColor::from(BUTTON_HOVER_COLOR);
            }
            Interaction::None => {
                *color = BackgroundColor::from(BUTTON_COLOR);
            }
        }
    }
}
