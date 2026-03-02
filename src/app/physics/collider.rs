#![allow(clippy::use_self)]
use bevy::prelude::*;
use bevy_rapier2d::prelude::Vect;
use serde::{Deserialize, Serialize};

/// Represents the collider (or lack thereof) of a sprite.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Asset, Reflect)]
pub enum ColliderData {
    NoCollider,
    Poly(Vec<Vect>),
}

impl Default for ColliderData {
    fn default() -> Self {
        Self::NoCollider
    }
}
