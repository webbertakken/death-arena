#![allow(clippy::use_self)]
use bevy_rapier2d::prelude::Vect;
use serde::{Deserialize, Serialize};

/// Represents the collider (or lack thereof) of a sprite.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, bevy::reflect::TypeUuid)]
#[uuid = "413be529-c123-ffdf-9db0-4b8b380a2c46"]
pub enum ColliderData {
    NoCollider,
    Poly(Vec<Vect>),
}

impl Default for ColliderData {
    fn default() -> Self {
        Self::NoCollider
    }
}
