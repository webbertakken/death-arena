# Development

## Setup

- Initiate lfs

```powershell
git lfs install
```

- Install Rust, using the `rustup` method ([docs](https://www.rust-lang.org/tools/install))

```powershell
choco install -y `
  rustup.install
```

- Restart terminal
- Install Bevy (game engine) prerequisites ([docs](https://bevyengine.org/learn/book/getting-started/setup/))

```powershell
rustup default stable-x86_64-pc-windows-gnu
cargo install -f cargo-binutils
rustup component add llvm-tools-preview
```

- Install web capability

```powershell
cargo install --locked trunk
rustup target add wasm32-unknown-unknown
```

- Run the tests.
  - This will install dependencies and dev-dependencies. 
  - The first time will take a few minutes.
  - This will automatically install rusty-hook.

```powershell
cargo test
```

## Run

Run the game with hot reload!

```powershell
trunk serve
```

## Tools

### Collider creator

#### Install

```powershell
cargo install rusty_engine --example collider
```

#### Run

```powershell
collider .\assets\textures\my-texture.png
```



