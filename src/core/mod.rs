use dotenvy::dotenv;

pub(crate) mod serde;

pub fn init() {
    dotenv().ok();
}
