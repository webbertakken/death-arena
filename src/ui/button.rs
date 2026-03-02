use crate::ui::prelude::*;
use bevy::prelude::*;

#[derive(Default)]
pub struct UiButton {}

impl UiButton {
    pub fn normal(&self) -> impl Bundle {
        let color = BUTTON_COLOR;
        (
            Button,
            Node {
                display: Display::Flex,
                width: Val::Percent(100.0),
                height: Val::Px(50.0),
                margin: UiRect::all(Val::Px(0.0)),
                padding: UiRect::all(Val::Px(8.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..Default::default()
            },
            BackgroundColor::from(color),
        )
    }
}

pub fn styles_system(mut query: Query<(&Interaction, &mut BackgroundColor), With<Button>>) {
    for (interaction, mut color) in query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
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
