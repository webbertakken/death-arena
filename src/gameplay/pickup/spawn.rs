use crate::gameplay::main::BOUNDS;
use crate::gameplay::pickup::{Pickup, PickupKind};
use bevy::prelude::*;

/// Z layer pickups render on: below the cars (player is at `z = 5`, opponents
/// at `z = 4`) so a car visibly drives over the collectible.
pub(super) const PICKUP_Z: f32 = 2.0;

/// The fixed scatter of collectibles across the arena floor.
///
/// Positions are kept well inside the arena bounds so a pickup never spawns
/// half-buried in the invisible wall.
#[must_use]
pub fn pickup_layout() -> Vec<(PickupKind, Vec2)> {
    let x = BOUNDS.x / 2.0 - 300.0;
    let y = BOUNDS.y / 2.0 - 300.0;
    vec![
        (PickupKind::Cash, Vec2::new(0.0, 0.0)),
        (PickupKind::Cash, Vec2::new(x * 0.45, 0.0)),
        (PickupKind::Cash, Vec2::new(-x * 0.45, 0.0)),
        (PickupKind::Nitro, Vec2::new(x * 0.75, 0.0)),
        (PickupKind::Nitro, Vec2::new(-x * 0.75, 0.0)),
        (PickupKind::Cash, Vec2::new(x, y)),
        (PickupKind::Cash, Vec2::new(-x, -y)),
        (PickupKind::Repair, Vec2::new(-x, y)),
        (PickupKind::Repair, Vec2::new(x, -y)),
        (PickupKind::Nitro, Vec2::new(0.0, y)),
        (PickupKind::Nitro, Vec2::new(0.0, -y)),
    ]
}

/// Scatters the [`pickup_layout`] across the arena when gameplay starts.
pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let texture = asset_server.load("textures/wrench.png");

    for (kind, position) in pickup_layout() {
        commands.spawn((
            Name::new("Pickup"),
            Pickup { kind },
            SpriteBundle {
                texture: texture.clone(),
                transform: Transform {
                    translation: position.extend(PICKUP_Z),
                    scale: Vec3::splat(0.15),
                    ..default()
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
    fn layout_keeps_every_pickup_inside_bounds() {
        let max_x = BOUNDS.x / 2.0;
        let max_y = BOUNDS.y / 2.0;
        for (_, position) in pickup_layout() {
            assert!(position.x.abs() < max_x, "x out of bounds: {}", position.x);
            assert!(position.y.abs() < max_y, "y out of bounds: {}", position.y);
        }
    }

    #[test]
    fn layout_is_not_empty() {
        assert!(!pickup_layout().is_empty());
    }

    #[test]
    fn layout_mirrors_each_pickup_kind_across_arena_centre() {
        let layout = pickup_layout();

        for (kind, position) in &layout {
            let mirrored_position = -*position;
            assert!(
                layout.iter().any(|(other_kind, other_position)| {
                    other_kind == kind && other_position.distance(mirrored_position) <= f32::EPSILON
                }),
                "missing mirrored {kind:?} pickup at {mirrored_position}"
            );
        }
    }

    #[test]
    fn layout_does_not_stack_pickups_on_one_position() {
        let layout = pickup_layout();

        for (index, (_, position)) in layout.iter().enumerate() {
            assert!(
                !layout
                    .iter()
                    .skip(index + 1)
                    .any(|(_, other_position)| other_position.distance(*position) <= f32::EPSILON),
                "duplicate pickup position at {position}"
            );
        }
    }

    #[test]
    fn layout_places_pickups_on_capture_lane() {
        let lane_pickups = pickup_layout()
            .into_iter()
            .filter(|(_, position)| {
                position.x.abs() > f32::EPSILON && position.y.abs() <= f32::EPSILON
            })
            .count();

        assert!(
            lane_pickups >= 2,
            "expected mirrored pickups on the central capture lane"
        );
    }

    #[test]
    fn layout_places_nitro_on_capture_lane() {
        let nitro_lane_pickups = pickup_layout()
            .into_iter()
            .filter(|(kind, position)| {
                *kind == PickupKind::Nitro
                    && position.x.abs() > f32::EPSILON
                    && position.y.abs() <= f32::EPSILON
            })
            .count();

        assert!(
            nitro_lane_pickups >= 2,
            "expected mirrored nitro pickups on the central capture lane"
        );
    }
}
