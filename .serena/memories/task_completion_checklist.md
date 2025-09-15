# Task Completion Checklist

## When Task is Completed

### Code Quality Checks
```bash
# Format code according to project standards
cargo fmt

# Check for common issues and style violations
cargo clippy

# Run all tests
cargo test

# Verify compilation
cargo check
```

### Build Verification
```bash
# Standard release build
cargo build --release

# Production build for target platform
cargo build --release --target=i686-pc-windows-msvc
```

### Pre-commit Checks
- Ensure no compilation errors
- Resolve all clippy warnings
- Verify tests pass
- Check that formatting is consistent (2 spaces, no tabs)
- Verify Windows compatibility (32-bit target)

### Documentation
- Update relevant comments if public APIs changed
- Document any Windows-specific considerations
- Note any breaking changes for plugin users

### Notes
- CI will automatically update MD5 checksums
- DLL output should be compatible with 伺か systems
- Consider Japanese text encoding implications