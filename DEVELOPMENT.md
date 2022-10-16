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

## Run for Web

#### Prerequisites
requires trunk and wasm32-unknown-unknown target:

```powershell
cargo install --locked trunk
rustup target add wasm32-unknown-unknown
```

#### Start

Start the web build:

```powershell
`trunk serve`
```

this will serve your app on 8080 and automatically rebuild + reload it after code changes.
