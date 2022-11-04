use bevy::asset::AssetServer;
use bevy::prelude::*;
use std::default;

mod button;
pub(crate) mod constants;
mod layout;
mod text;

pub struct UiComponents {
    pub layout: layout::Layout,
    pub button: button::Button,
    pub text: text::TextBox,
}

pub fn ui(asset_server: &Res<AssetServer>) -> UiComponents {
    let font = asset_server.load("fonts/FiraSans-Bold.ttf");

    let layout = layout::Layout::default();
    let button = button::setup();
    let text = text::TextBox { font };

    UiComponents {
        layout,
        button,
        text,
    }
}
