use bevy::prelude::*;
use bevy::ui::FocusPolicy;

pub struct Layout {
    default_direction: FlexDirection,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            default_direction: FlexDirection::Column,
        }
    }
}

impl Layout {
    pub fn spacer(&self, size: f32) -> impl Bundle {
        (
            Node {
                width: Val::Px(size),
                height: Val::Px(size),
                flex_grow: 0.0,
                ..Default::default()
            },
            BackgroundColor::from(Color::NONE),
        )
    }

    pub fn flex(&self, color: Color) -> impl Bundle {
        (
            Node {
                flex_direction: self.default_direction,
                margin: UiRect::all(Val::Auto),
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(40.0)),
                min_width: Val::Px(400.0),
                min_height: Val::Px(400.0),
                max_width: Val::Px(400.0),
                max_height: Val::Px(800.0),
                ..default()
            },
            FocusPolicy::Pass,
            BackgroundColor::from(color),
        )
    }
}
