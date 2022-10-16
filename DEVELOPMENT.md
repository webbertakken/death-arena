# Development

## Setup

- Initiate lfs

```powershell
git lfs install
```

- Install Rust, using the `rustup` method ([docs](https://www.rust-lang.org/tools/install))

```powershell
choco install rustup.install
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

- Run the game. The first time will take a few minutes, because dependencies have to be compiled.

## Run

Run the game with hot reload!

```powershell
trunk serve
```
