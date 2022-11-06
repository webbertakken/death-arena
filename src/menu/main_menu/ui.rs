use crate::menu::main_menu::ButtonAction;
use crate::ui::prelude::*;
use crate::ui::{ui, Atoms};
use bevy::prelude::*;
use bevy::sprite::Rect;
use bevy_inspector_egui::egui::style::Selection;

#[derive(Component)]
pub struct MainMenu;

pub fn hide(mut commands: Commands, query: Query<Entity, With<MainMenu>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

pub fn show(mut commands: Commands, asset_server: Res<AssetServer>) {
    let Atoms {
        layout,
        button,
        text,
    } = ui(&asset_server);

    // Main menu
    commands
        .spawn_bundle(layout.flex(MENU_COLOR))
        .insert(Name::new("Menu (main)"))
        .insert(MainMenu)
        .with_children(|parent| {
            // Title
            parent
                .spawn_bundle(text.title("Death Arena", TEXT_COLOR))
                .insert(Name::new("Title"));

            // Multiplayer button
            parent
                .spawn_bundle(button.normal())
                .insert(Name::new("Button (Multiplayer)"))
                .insert(ButtonAction::Multiplayer)
                .with_children(|parent| {
                    parent.spawn_bundle(text.button("Quick play", TEXT_COLOR));
                });

            // Spacer
            parent.spawn_bundle(layout.spacer(8.0));

            // Play button
            parent
                .spawn_bundle(button.normal())
                .insert(Name::new("Button (Career)"))
                .insert(ButtonAction::Career)
                .with_children(|parent| {
                    parent.spawn_bundle(text.button("Career (WIP)", TEXT_COLOR));
                });
        });
}
