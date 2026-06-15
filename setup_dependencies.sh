#!/usr/bin/env bash
set -euo pipefail

# Install system dependencies for Death Arena (Bevy and audio)
# This script is intended for Ubuntu/Debian systems.

echo "Updating package lists..."
sudo apt-get update

echo "Installing required system dependencies..."
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

echo "Installation complete."
