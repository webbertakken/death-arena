use crate::gameplay::player::Player;
use bevy::prelude::*;
use bevy::render::camera::Viewport;
use bevy::render::view::ViewPlugin;

#[derive(Component)]
pub struct MainCamera;

// #[derive(Component)]
// struct LeftCamera;
//
// #[derive(Component)]
// struct RightCamera;

pub fn setup(mut commands: Commands) {
    // 2D orthographic camera
    commands
        .spawn_bundle(Camera2dBundle::default())
        .insert(MainCamera);

    // Light
    // commands.spawn_bundle(DirectionalLightBundle {
    //     transform: Transform::from_rotation(Quat::from_euler(
    //         EulerRot::ZYX,
    //         0.0,
    //         1.0,
    //         -std::f32::consts::FRAC_PI_4,
    //     )),
    //     directional_light: DirectionalLight {
    //         shadows_enabled: true,
    //         ..default()
    //     },
    //     ..default()
    // });

    // Split screen example: https://github.com/bevyengine/bevy/blob/latest/examples/3d/split_screen.rs
}

pub fn camera_follows_player_system(
    player_query: Query<&Transform, (With<Player>, Without<MainCamera>)>,
    mut camera_query: Query<&mut Transform, With<MainCamera>>,
) {
    let player_transform = player_query.single();
    let mut camera_transform = camera_query.single_mut();
    camera_transform.translation = player_transform.translation;
}