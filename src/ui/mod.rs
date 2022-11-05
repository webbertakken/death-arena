use bevy::asset::AssetServer;
use bevy::prelude::*;
use std::default;

pub(crate) mod button;
pub(crate) mod constants;
mod layout;
mod text;

#[derive(Default)]
pub struct UserInterfacePlugin;

impl Plugin for UserInterfacePlugin {
    fn build(&self, app: &mut App) {
        app.add_system(button::styles_system);
    }
}

pub struct UiComponents {
    pub layout: layout::Layout,
    pub button: button::UiButton,
    pub text: text::TextBox,
}

pub fn ui(asset_server: &Res<AssetServer>) -> UiComponents {
    let font = asset_server.load("fonts/FiraSans-Bold.ttf");

    let layout = layout::Layout::default();
    let button = button::UiButton::default();
    let text = text::TextBox { font };

    UiComponents {
        layout,
        button,
        text,
    }
}
