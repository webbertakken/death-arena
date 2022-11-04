use bevy::prelude::*;

pub struct Layout {
    default_direction: FlexDirection,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            default_direction: FlexDirection::ColumnReverse,
        }
    }
}

impl Layout {
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

            color: UiColor::from(color),
            ..default()
        }
    }
}
