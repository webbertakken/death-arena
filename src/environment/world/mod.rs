use bevy::prelude::*;

use crate::{App, Input, KeyCode, Plugin, Query, Res, Transform, Vec3};
use bevy::{math::Vec3Swizzles, time::FixedTimestep};
mod arena;

#[derive(Default)]
pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(arena::setup).run();
    }
}
