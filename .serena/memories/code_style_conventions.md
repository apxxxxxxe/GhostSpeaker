# Code Style and Conventions

## Formatting (rustfmt.toml)
- **Indentation**: 2 spaces (no hard tabs)
- **Line length**: Default Rust standards
- **Formatting tool**: `cargo fmt`

## Naming Conventions
- **Functions/variables**: snake_case
- **Types/structs**: PascalCase  
- **Constants**: SCREAMING_SNAKE_CASE
- **Modules**: snake_case

## Code Organization
- **Modules**: Organized by functionality (engine/, events/, plugin/)
- **Traits**: Used for engine abstraction (`async-trait`)
- **Error Handling**: Result<T, E> patterns
- **Async**: tokio-based async/await

## Architecture Patterns
- **Plugin Pattern**: Engine implementations via traits
- **Async/Await**: Heavy use for HTTP TTS requests
- **Resource Management**: Careful memory handling for Windows DLL
- **Configuration**: YAML-based settings (vars.yaml)

## Documentation
- Use Rust doc comments (///) for public APIs
- Focus on functionality and Windows-specific considerations
- Document Japanese text encoding handling

## Windows-Specific Considerations  
- 32-bit compatibility required
- Windows API usage for process management
- Japanese text encoding (UTF-8/ANSI fallback)
- DLL export requirements