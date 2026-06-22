# ScreamWire

A lightweight, low-latency audio sender for PipeWire, compatible with the
[Scream](https://github.com/duncanthrax/scream) protocol.
Route any application's audio to a virtual output device or capture from an
existing sink – it will be streamed over UDP (multicast or unicast) to any
Scream receiver.

## Features

- **Virtual Sink** – automatically creates a `ScreamWire` output device when
  no explicit sink is given. Just select it in your mixer and audio flows to
  the network.
- **Existing Sink Capture** – alternatively, capture the monitor of any
  already-available output (`--sink <name>`). Use `--list-sinks` to see what's
  available.
- **Voice Activity Detection** – pauses transmission after a configurable
  silence period (1 s by default) and resumes instantly when sound returns,
  saving bandwidth and CPU.
- **Flexible Configuration** – all parameters can be set via CLI flags or a
  TOML file. Defaults are sensible.
  - XDG‑aware: `~/.config/screamwire/config.toml` is loaded automatically if
    present.
  - Generate a default config with `--generate-config`.
- **Robust PipeWire Integration** – uses `core.sync` to reliably enumerate
  sinks, handles both capture and playback streams, and displays a volume
  indicator in `pavucontrol`.
- **Logging** – `--verbose` enables detailed diagnostics; normal operation is
  quiet.
- **Systemd User Unit** – included in `contrib/systemd/` for automatic startup
  with your session.

## Quick Start

```bash
# Build and install
cargo install --path . --root ~/.cargo

# Run (virtual sink, multicast target 239.255.77.77:4010)
screamwire

# List available sinks and capture from one
screamwire --list-sinks
screamwire --sink alsa_output.pci-0000_00_1f.3.analog-stereo
```

## Configuration

Create `~/.config/screamwire/config.toml` (or use `--generate-config`) and
adjust the settings:

```toml
target_addr = "239.255.77.77:4010"
sender_bind_addr = "0.0.0.0:0"
rate = 48000
channels = 2
vad_threshold = 1
silence_packets = 167
ring_buffer_packets = 10
active_sleep_ms = 4
idle_sleep_ms = 30
```

All options can also be given on the command line (run `screamwire --help`).

## Building from Source

```bash
git clone https://github.com/avalak/screamwire.git
cd screamwire
cargo build --release
```

The binary will be in `target/release/screamwire`.

## License

MIT (see `LICENSE`).

## Acknowledgements

- [Scream](https://github.com/duncanthrax/scream) – the original protocol.
- [PipeWire](https://pipewire.org) – the modern Linux audio stack.
- [pipewire-rs](https://gitlab.freedesktop.org/pipewire/pipewire-rs) – Rust bindings.
