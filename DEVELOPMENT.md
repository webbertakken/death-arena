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

### Linux (Ubuntu/Debian)

To install the required system dependencies for Bevy and audio, run the provided setup script:

```bash
./setup_dependencies.sh
```

Or manually (requires `sudo`):

```bash
sudo apt-get update
sudo apt-get install -y \
    pkg-config \
    libasound2-dev \
    libudev-dev \
    libx11-dev \
    libxcursor-dev \
    libxinerama-dev \
    libxrandr-dev \
    libxi-dev \
    libgl1-mesa-dev \
    libegl1-mesa-dev
```

### Windows

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

## Troubleshooting

If you encounter build errors related to missing system libraries, ensure all dependencies are installed.

### Missing 'alsa' or 'asound' library
If you see an error like `Could not find libasound.so` or `alsa` related errors during compilation, it's likely because the ALSA development headers are missing. 

On Ubuntu/Debian, install it with:
```bash
sudo apt-get install libasound2-dev
```

### Other common errors
Most system-level dependency issues can be resolved by running the provided setup script:
```bash
./setup_dependencies.sh
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
