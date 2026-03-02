use crate::gameplay::player::Player;
use bevy::prelude::*;

#[derive(Component)]
pub struct MainCamera;

pub fn setup(mut commands: Commands) {
    // 2D orthographic camera
    commands.spawn((
        Name::new("MainCamera"),
        MainCamera,
        Camera2d,
    ));
}

pub fn camera_follows_player_system(
    player_query: Query<&Transform, (With<Player>, Without<MainCamera>)>,
    mut camera_query: Query<&mut Transform, With<MainCamera>>,
) {
    let player_transform = player_query.single();
    let mut camera_transform = camera_query.single_mut();
    camera_transform.translation = player_transform.translation;
}
