use crate::ui::constants::*;
use crate::ui::{ui, UiComponents};
use bevy::prelude::*;
use bevy::sprite::Rect;
use bevy_inspector_egui::egui::style::Selection;

#[derive(Component)]
enum MenuButtonAction {
    Play,
    Quit,
    MainMenu,
    Restart,
}

#[derive(Component)]
pub struct MainMenu;

pub fn show(mut commands: Commands, asset_server: Res<AssetServer>) {
    let UiComponents {
        layout,
        button,
        text,
    } = ui(&asset_server);

    // Main menu
    commands
        .spawn_bundle(layout.flex(MENU_COLOR))
        .insert(MainMenu)
        .with_children(|parent| {
            // Title
            parent.spawn_bundle(text.title("Death Arena", TEXT_COLOR));

            // Play button
            parent
                .spawn_bundle(button.create(BUTTON_COLOR))
                .insert(MenuButtonAction::Play)
                .with_children(|parent| {
                    parent.spawn_bundle(text.button("Career", TEXT_COLOR));
                });
        });
}
