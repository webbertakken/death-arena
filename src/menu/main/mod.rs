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
pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(MenuState::Main), ui::show);
        app.add_systems(
            Update,
            on_update_main_menu.run_if(in_state(MenuState::Main)),
        );
        app.add_systems(OnExit(MenuState::Main), ui::hide);
    }
}

pub type FilterButtonsThatChanged = (Changed<Interaction>, With<Button>);

pub fn on_update_main_menu(
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
