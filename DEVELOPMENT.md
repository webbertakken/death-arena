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
cargo install -f cargo-binutils
rustup component add llvm-tools-preview
```

- Run the game. The first time will take a few minutes.

## Run

```powershell
cargo run
```
