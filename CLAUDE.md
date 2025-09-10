# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

ユーザとの対話は日本語で行うこと。

## Project Overview

GhostSpeaker is a Rust-based SHIORI plugin for 伺か (Ukagaka) that enables text-to-speech functionality for ghost characters using various TTS engines. It's built as a Windows DLL (`ghost_speaker.dll`) that integrates with the 伺か ecosystem.

## Build Commands

### Standard Development
```bash
# Build in release mode (standard)
cargo build --release

# Build for 32-bit Windows (production target)
cargo build --release --target=i686-pc-windows-msvc

# Run tests
cargo test

# Format code (uses 2-space indentation, no hard tabs)
cargo fmt

# Check for clippy warnings
cargo clippy
```

### Production Build
The project is configured to build 32-bit Windows DLLs for compatibility with 伺か systems:
- Target: `i686-pc-windows-msvc`
- Output: `ghost_speaker.dll` (copied to project root)
- CI builds automatically update MD5 checksums via GitHub Actions

## Architecture

### Core Components

**Plugin Interface (`src/lib.rs`)**
- SHIORI plugin entry points: `load()`, `loadu()`, `unload()`, `request()`
- Handles UTF-8/ANSI encoding conversion for Japanese text
- Manages plugin lifecycle and panic handling

**Engine System (`src/engine/`)**
- Modular TTS engine support via trait system
- Supported engines: COEIROINK (v1/v2), VOICEVOX, LMROID, SHAREVOX, ITVOICE, AivisSpeech, 棒読みちゃん
- Each engine has dedicated modules with `speaker.rs` and `predict.rs`
- HTTP-based communication with TTS services (except 棒読みちゃん)

**Event Handling (`src/events/`)**
- `menu.rs`: Plugin menu system integration
- `periodic.rs`: Timer-based events
- `other_ghost.rs`: Inter-ghost communication
- `common.rs`: Shared event utilities

**Audio System**
- `src/player.rs`: Audio playback using rodio
- `src/queue.rs`: Speech queue management
- `src/speaker.rs`: Voice/speaker configuration

**Configuration (`src/variables/`)**
- `rawvariables.rs`: Persistent settings storage
- Saves to `vars.yaml` in plugin directory
- Manages per-ghost voice assignments and engine settings

### Key Design Patterns

**Async Architecture**
- Heavy use of `tokio` for async HTTP requests to TTS engines
- `async-trait` for engine abstraction
- Queue system for managing concurrent speech requests

**Plugin Integration**
- SHIORI protocol implementation for 伺か communication
- Windows-specific memory management via `shiori_hglobal`
- Japanese text encoding support (UTF-8/ANSI fallback)

**Engine Abstraction**
- Trait-based design allows easy addition of new TTS engines
- Each engine implements speaker discovery and speech synthesis
- Consistent API despite different underlying protocols

## Development Notes

### Windows-Specific Considerations
- Built for 32-bit Windows compatibility
- Uses Windows API for process management (`CreateProcessW`)
- Character encoding handling for Japanese text is critical

### TTS Engine Integration
- Most engines use REST APIs on localhost ports
- 棒読みちゃん uses a different protocol
- Engines must be running before plugin can connect
- Each engine has unique speaker/voice identification schemes

### Configuration Files
- `vars.yaml`: User settings and voice assignments
- `descript.txt`: Plugin metadata for 伺か
- `rustfmt.toml`: Code formatting (2 spaces, no hard tabs)

### Testing and CI
- GitHub Actions builds on Windows
- Auto-updates MD5 checksums for distribution
- Builds target i686-pc-windows-msvc specifically
