use bevy::prelude::*;

#[derive(Default)]
pub struct GenericSystemsPlugin;

impl Plugin for GenericSystemsPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(rotator);
    }
}

#[derive(Component)]
pub struct AlwaysRotates;

fn rotator(mut q_rotates: Query<&mut Transform, With<AlwaysRotates>>, time: Res<Time>) {
    for mut transform in &mut q_rotates {
        transform.rotation *= Quat::from_rotation_z(1.0 * time.delta_seconds());
    }
}
