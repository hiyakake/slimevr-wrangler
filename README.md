<div align="center">

# SlimeVR Wrangler (VMT Output)

[![Discord Server](https://img.shields.io/discord/817184208525983775?color=7389D8&label=Discord%20&logo=discord&logoColor=FFFFFF)](https://discord.gg/slimevr)
</div>

Use Joy-Cons as rotation trackers and stream their pose data to **VMT (Virtual Motion Tracker)** via OSC.
This lets you use Joy-Cons as SteamVR virtual trackers through VMT.

![Screenshot of the app running and tracking a single Joy-Con](screenshot.png)

## Runtime Setup (VMT)
You need Bluetooth on your computer.

1. Install and start **VMT (Virtual Motion Tracker)**.
2. Start SteamVR.
3. Connect Joy-Cons to your computer ([Windows pairing guide](https://www.digitaltrends.com/gaming/how-to-connect-a-nintendo-switch-controller-to-a-pc/)).
4. Start `slimevr-wrangler`.
5. Open **Settings** and confirm `VMT OSC address` (default: `127.0.0.1:39570`).
6. Return to the main view and wait for Joy-Cons to appear.

### Tracker behavior
- Position is fixed to `(0, 0, 0)`.
- Rotation (quaternion) is streamed from Joy-Con IMU data.
- This is intended to be a **rotation-only tracker** pipeline.

## Build from source

### Prerequisites
- Rust toolchain (stable): https://rustup.rs
- Platform dependencies for `hidapi`:
  - **Ubuntu/Debian**: `sudo apt install libudev-dev pkg-config`
  - **Fedora**: `sudo dnf install systemd-devel pkgconf-pkg-config`
  - **Arch**: `sudo pacman -S systemd pkgconf`

### Debug build
```bash
cargo build
```
Binary path:
- Linux/macOS: `target/debug/slimevr-wrangler`
- Windows: `target\\debug\\slimevr-wrangler.exe`

### Release build
```bash
cargo build --release
```
Binary path:
- Linux/macOS: `target/release/slimevr-wrangler`
- Windows: `target\\release\\slimevr-wrangler.exe`

### Run directly
```bash
cargo run --release
```

### Mounting

Attach the Joy-Cons in the direction that works best for your body placement.

Keep the joystick pointed outwards, it should not poke into your skin.

After connecting the Joy-Con's in the program, rotate them in the program to be the same rotation as they are if you are standing up.

## Issues

Many! This is a **alpha** version, and there's no guarantees about anything.

* Rotation tracking is bad! - Yup, sorry. In the future there will be settings to help fine tune tracking.
* It stops tracking when I turn around! - Bluetooth does not have a good range, you might have better luck with a different bluetooth adapter.
* Probably more.

### My Joy-Con's are connected in the Windows bluetooth menu but won't show up!

This is a problem that might be related to a newer Windows update. Try this, and it might fix it:
* Go to the Windows Setting app -> Bluetooth & other devices.
* Press on the Joy-Con that won't connect. Press "Remove device".
* Pair the device again. It should now show up.

# License
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version 2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.

<sup>Old versions using the rust package "ahrs" are licensed with GPL v2, check the git history for the license on your chosen commit.</sup>
