#![allow(clippy::unused_self)]

use bevy::asset::AssetServer;
use bevy::prelude::*;
use std::default;

pub(crate) mod button;
mod layout;
pub(crate) mod prelude;
mod text;

#[derive(Default)]
pub struct UserInterfacePlugin;

impl Plugin for UserInterfacePlugin {
    fn build(&self, app: &mut App) {
        app.add_system(button::styles_system);
    }
}

pub struct Atoms {
    pub layout: layout::Layout,
    pub button: button::UiButton,
    pub text: text::TextBox,
}

#[must_use]
pub fn ui(asset_server: &Res<AssetServer>) -> Atoms {
    let font = asset_server.load("fonts/FiraSans-Bold.ttf");

    let layout = layout::Layout::default();
    let button = button::UiButton::default();
    let text = text::TextBox { font };

    Atoms {
        layout,
        button,
        text,
    }
}
