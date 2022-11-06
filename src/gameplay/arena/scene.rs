use crate::gameplay::arena::scene_loader::Sprite;
use bevy::prelude::Vec3;
use serde::{de, Deserialize, Deserializer};
use serde_json::Value;

#[derive(Debug, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Scale {
    #[serde(deserialize_with = "string_to_float")]
    pub x: f32,
    #[serde(deserialize_with = "string_to_float")]
    pub y: f32,
    #[serde(deserialize_with = "string_to_float")]
    pub z: f32,
}

fn string_to_float<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::String(s) => s.parse().map_err(de::Error::custom)?,
        _ => return Err(de::Error::custom("wrong type")),
    })
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpriteDefaults {
    /// Position in 2D space
    pub position: Position,
    /// Rotation in 2D space (Z-axis)
    pub rotation: f32,
    /// Scale in 2D space
    pub scale: Scale,
    /// Opacity
    pub opacity: f32,
    /// Whether you can drag the sprite
    locked: bool,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpriteData {
    // Unique name of the sprite
    pub id: String,
    /// Relative path to the asset
    pub relative_path: String,
    /// Position in 2D space
    pub position: Position,
    /// Rotation in 2D space (Z-axis)
    pub rotation: f32,
    /// Scale in 2D space
    pub scale: Scale,
    /// Opacity
    pub opacity: f32,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Canvas {
    pub(crate) sprites: Vec<SpriteData>,
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
    pub default_properties: SpriteDefaults,
}
