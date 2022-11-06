use serde::{de, Deserialize, Deserializer};
use serde_json::Value;

#[allow(clippy::cast_possible_truncation)]
pub fn parse_float<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::String(s) => s.parse().map_err(de::Error::custom)?,
        Value::Number(num) => {
            num.as_f64()
                .ok_or_else(|| de::Error::custom("Invalid number"))? as f32
        }
        _ => return Err(de::Error::custom("wrong type")),
    })
}
