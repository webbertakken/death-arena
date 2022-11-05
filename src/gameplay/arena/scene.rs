use bevy::prelude::Vec3;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    x: i32,
    y: i32,
    z: i32,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scale {
    x: String,
    y: String,
    z: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpriteDefaults {
    /// Position in 2D space
    position: Position,
    /// Rotation in 2D space (Z-axis)
    rotation: i32,
    /// Scale in 2D space
    scale: Scale,
    /// Opacity
    opacity: f32,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpriteDefaultsWithLocked {
    #[serde(flatten)]
    sprite_defaults: SpriteDefaults,
    /// Whether you can drag the sprite
    locked: bool,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpriteData {
    // Unique name of the sprite
    id: String,
    /// Relative path to the asset
    relative_path: String,
    /// Include all fields from the defaults as well
    #[serde(flatten)]
    sprite_defaults: SpriteDefaults,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Canvas {
    sprites: Vec<SpriteData>,
}

#[derive(Debug, serde::Deserialize, bevy::reflect::TypeUuid)]
#[serde(rename_all = "camelCase")]
#[uuid = "413be529-bfff-f1b3-9db0-4b8b380a2c46"]
pub struct Scene {
    pub name: String,
    pub version: String,
    pub description: String,
    pub assets_relative_path: String,
    pub canvas: Canvas,
    pub default_properties: SpriteDefaultsWithLocked,
}
