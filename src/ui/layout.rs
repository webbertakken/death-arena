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
    pub fn spacer(&self, size: f32) -> NodeBundle {
        NodeBundle {
            style: Style {
                size: Size::new(Val::Px(size), Val::Px(size)),
                flex_grow: 0.0,
                ..Default::default()
            },
            background_color: Color::NONE.into(),
            ..Default::default()
        }
    }

    pub fn flex(&self, color: Color) -> NodeBundle {
        NodeBundle {
            style: Style {
                flex_direction: self.default_direction,
                margin: UiRect::all(Val::Auto),
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(40.0)),
                min_size: Size::new(Val::Px(400.0), Val::Px(400.0)),
                max_size: Size::new(Val::Px(400.0), Val::Px(800.0)),
                ..default()
            },
            focus_policy: FocusPolicy::Pass,
            background_color: BackgroundColor::from(color),
            ..default()
        }
    }
}
