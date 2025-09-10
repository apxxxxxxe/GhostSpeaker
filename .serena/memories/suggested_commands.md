# Suggested Commands for GhostSpeaker Development

## Build Commands
```bash
# Standard development build
cargo build --release

# Production build (32-bit Windows target)
cargo build --release --target=i686-pc-windows-msvc

# Development build (debug)
cargo build
```

## Testing and Quality
```bash
# Run tests
cargo test

# Format code (2 spaces, no hard tabs)
cargo fmt

# Check for clippy warnings
cargo clippy

# Check compilation without building
cargo check
```

## Windows System Commands
```cmd
# List files
dir
# or use PowerShell
ls

# Change directory
cd <path>

# Find files (PowerShell)
Get-ChildItem -Recurse -Name "*.rs"

# Search in files (PowerShell)
Select-String -Path "*.rs" -Pattern "pattern"
```

## Git Commands
```bash
git status
git add .
git commit -m "message"
git push
git pull
```

## Project-Specific Notes
- Output DLL: `ghost_speaker.dll` (copied to project root)
- Configuration: `vars.yaml` in plugin directory
- Target compatibility: 32-bit Windows systems
- CI builds automatically update MD5 checksums