# GhostSpeaker Project Overview

## Purpose
GhostSpeaker is a Rust-based SHIORI plugin for 伺か (Ukagaka) that enables text-to-speech functionality for ghost characters using various TTS engines. It's built as a Windows DLL (`ghost_speaker.dll`) that integrates with the 伺か ecosystem.

## Tech Stack
- **Language**: Rust (Edition 2021)
- **Target**: Windows DLL (32-bit compatibility: `i686-pc-windows-msvc`)
- **Key Dependencies**:
  - `tokio`: Async runtime for HTTP requests
  - `rodio`: Audio playback
  - `reqwest`: HTTP client for TTS engines
  - `shiorust`: SHIORI protocol implementation
  - `serde`: Serialization (YAML/JSON config)
  - `winapi`: Windows API bindings
  - `async-trait`: Trait abstraction for engines

## Codebase Structure
```
src/
├── lib.rs              # Plugin entry points (load, unload, request)
├── engine/             # TTS engine implementations
│   ├── bouyomichan/    # 棒読みちゃん support
│   ├── coeiroink_v2/   # COEIROINK v2 support  
│   └── voicevox_family/ # VOICEVOX, LMROID, SHAREVOX, etc.
├── events/             # Event handling (menu, periodic, inter-ghost)
├── plugin/             # SHIORI request/response handling
├── variables/          # Configuration management (vars.yaml)
├── player.rs           # Audio playback
├── queue.rs            # Speech queue management
└── speaker.rs          # Voice/speaker configuration
```

## Supported TTS Engines
- COEIROINK (v1/v2)
- VOICEVOX
- LMROID  
- SHAREVOX
- ITVOICE
- AivisSpeech
- 棒読みちゃん

## Key Architecture Patterns
- **Async Architecture**: Heavy use of tokio for HTTP TTS requests
- **Engine Abstraction**: Trait-based design for TTS engines
- **Plugin Integration**: SHIORI protocol for 伺か communication
- **Windows Compatibility**: 32-bit DLL with Japanese text encoding support