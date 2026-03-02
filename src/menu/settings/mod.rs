use crate::menu::MenuState;
use crate::{App, AppState, Plugin};
use bevy::prelude::*;

mod ui;

#[derive(Component)]
pub enum ButtonAction {
    Career,
    Multiplayer,
}

#[derive(Default)]
pub struct SettingsMenuPlugin;

impl Plugin for SettingsMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(MenuState::Settings), ui::show);
        app.add_systems(Update, on_update.run_if(in_state(MenuState::Settings)));
        app.add_systems(OnExit(MenuState::Settings), ui::hide);
    }
}

pub type FilterButtonsThatChanged = (Changed<Interaction>, With<Button>);

pub fn on_update(
    interaction_query: Query<(&Interaction, &ButtonAction), FilterButtonsThatChanged>,
    mut next_menu_state: ResMut<NextState<MenuState>>,
    mut next_app_state: ResMut<NextState<AppState>>,
) {
    for (interaction, action) in &interaction_query {
        if *interaction == Interaction::Pressed {
            match action {
                ButtonAction::Career => {
                    info!("Career button clicked");
                    next_menu_state.set(MenuState::Garage);
                }
                ButtonAction::Multiplayer => {
                    info!("Multiplayer button clicked");
                    next_menu_state.set(MenuState::Hidden);
                    next_app_state.set(AppState::Loading);
                }
            }
        }
    }
}
