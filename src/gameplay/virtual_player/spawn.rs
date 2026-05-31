use crate::gameplay::main::BOUNDS;
use crate::gameplay::virtual_player::ai::AiTeam;
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

#[derive(Debug, Clone, Copy, PartialEq)]
struct VirtualPlayerSpawn {
    name: &'static str,
    team: AiTeam,
    start_waypoint: usize,
    translation: Vec3,
}

const fn spawn_roster() -> [VirtualPlayerSpawn; 4] {
    [
        VirtualPlayerSpawn {
            name: "Teammate 1",
            team: AiTeam::Blue,
            start_waypoint: 3,
            translation: Vec3::new(-430.0, 200.0, 4.0),
        },
        VirtualPlayerSpawn {
            name: "Opponent 1",
            team: AiTeam::Red,
            start_waypoint: 0,
            translation: Vec3::new(430.0, 200.0, 4.0),
        },
        VirtualPlayerSpawn {
            name: "Opponent 2",
            team: AiTeam::Red,
            start_waypoint: 1,
            translation: Vec3::new(0.0, 380.0, 4.0),
        },
        VirtualPlayerSpawn {
            name: "Opponent 3",
            team: AiTeam::Red,
            start_waypoint: 2,
            translation: Vec3::new(-430.0, -200.0, 4.0),
        },
    ]
}

/// Spawns a small grid of virtual CTF drivers that patrol the arena.
pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let chassis = asset_server.load("textures/car1/chassis1.png");
    let route = arena_patrol_route();

    for spawn in spawn_roster() {
        commands.spawn((
            Name::new(spawn.name),
            VirtualPlayer {
                team: spawn.team,
                movement_speed: 420.0,
                rotation_speed: f32::to_radians(300.0),
                waypoints: route.clone(),
                current_waypoint: spawn.start_waypoint,
            },
            SpriteBundle {
                texture: chassis.clone(),
                transform: Transform {
                    translation: spawn.translation,
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

    #[test]
    fn roster_includes_player_team_and_opponents() {
        let roster = spawn_roster();

        assert!(roster.iter().any(|spawn| spawn.team == AiTeam::Blue));
        assert!(roster.iter().any(|spawn| spawn.team == AiTeam::Red));
    }
}
