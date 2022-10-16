use crate::{App, Plugin};

#[derive(Default)]
pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, _app: &mut App) {
        // add things to your app  here
    }
}
