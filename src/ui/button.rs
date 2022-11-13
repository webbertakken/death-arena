use crate::ui::prelude::*;
use bevy::prelude::*;

#[derive(Default)]
pub struct UiButton {}

impl UiButton {
    pub fn normal(&self) -> ButtonBundle {
        let color = BUTTON_COLOR;
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

pub fn styles_system(mut query: Query<(&Interaction, &mut UiColor), With<Button>>) {
    for (interaction, mut color) in query.iter_mut() {
        match *interaction {
            Interaction::Clicked => {
                *color = UiColor::from(BUTTON_ACTIVE_COLOR);
            }
            Interaction::Hovered => {
                *color = UiColor::from(BUTTON_HOVER_COLOR);
            }
            Interaction::None => {
                *color = UiColor::from(BUTTON_COLOR);
            }
        }
    }
}
