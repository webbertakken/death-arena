use crate::gameplay::main::BOUNDS;
use crate::gameplay::virtual_player::VirtualPlayer;
use bevy::prelude::*;

/// Returns a rectangular patrol route that hugs the inside of the arena, giving
/// opponents the classic Death Rally "lapping the track" feel.
pub fn arena_patrol_route() -> Vec<Vec2> {
    let x = BOUNDS.x / 2.0 - 250.0;
    let y = BOUNDS.y / 2.0 - 250.0;
    vec![
        Vec2::new(x, y),
        Vec2::new(-x, y),
        Vec2::new(-x, -y),
        Vec2::new(x, -y),
    ]
}

/// Spawns a small grid of virtual opponents that patrol the arena.
pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let chassis = asset_server.load("textures/car1/chassis1.png");
    let route = arena_patrol_route();

    // Stagger opponents along the route so they do not stack on spawn.
    let opponents = [
        ("Opponent 1", 0, Vec3::new(430.0, 200.0, 4.0)),
        ("Opponent 2", 1, Vec3::new(0.0, 380.0, 4.0)),
        ("Opponent 3", 2, Vec3::new(-430.0, -200.0, 4.0)),
    ];

    for (name, start_waypoint, translation) in opponents {
        commands.spawn((
            Name::new(name),
            VirtualPlayer {
                movement_speed: 420.0,
                rotation_speed: f32::to_radians(300.0),
                waypoints: route.clone(),
                current_waypoint: start_waypoint,
            },
            SpriteBundle {
                texture: chassis.clone(),
                transform: Transform {
                    translation,
                    rotation: Quat::from_rotation_z(0.0),
                    scale: Vec3::new(0.2, 0.2, 0.2),
                },
                ..default()
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patrol_route_has_four_corners_inside_bounds() {
        let route = arena_patrol_route();
        assert_eq!(route.len(), 4);
        let max_x = BOUNDS.x / 2.0;
        let max_y = BOUNDS.y / 2.0;
        for point in route {
            assert!(point.x.abs() < max_x, "x out of bounds: {}", point.x);
            assert!(point.y.abs() < max_y, "y out of bounds: {}", point.y);
        }
    }
}
