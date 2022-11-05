use crate::menu::MenuState;
use crate::{App, AppState, Input, KeyCode, Plugin, Query, Res, Transform, Vec3};
use bevy::prelude::*;
use bevy_ecs::query::WorldQuery;
use bevy_kira_audio::prelude::*;

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
        // Enter
        app.add_system_set(SystemSet::on_enter(MenuState::Main).with_system(ui::show));

        // Update
        app.add_system_set(SystemSet::on_update(MenuState::Main).with_system(on_update_main_menu));

        // Exit
        app.add_system_set(SystemSet::on_exit(MenuState::Main).with_system(ui::hide));
    }
}

pub type FilterButtonsThatChanged = (Changed<Interaction>, With<Button>);

pub fn on_update_main_menu(
    interaction_query: Query<(&Interaction, &ButtonAction), FilterButtonsThatChanged>,
    mut menu_state: ResMut<State<MenuState>>,
    mut app_state: ResMut<State<AppState>>,
) {
    for (interaction, action) in &interaction_query {
        if *interaction == Interaction::Clicked {
            match action {
                ButtonAction::Career => {
                    info!("Career button clicked");
                    menu_state.overwrite_set(MenuState::Garage).unwrap();
                }
                ButtonAction::Multiplayer => {
                    info!("Multiplayer button clicked");
                    menu_state.overwrite_set(MenuState::Hidden).unwrap();
                    app_state.overwrite_set(AppState::InGame).unwrap();
                }
            }
        }
    }
}
