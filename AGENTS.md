# Agent Guidelines for cathode8

This document provides guidelines for AI agents working on the cathode8 NES emulator codebase.

## Build & Test Commands

### Basic Commands
```bash
# Build the project
cargo build
cargo build --release

# Run all tests
cargo test

# Run a single test by name
cargo test mapper2_keeps_last_bank_fixed
cargo test mapper4_irq_a12_edge_filtering

# Run tests in a specific file
cargo test --test <test_name>

# Run tests with output
cargo test -- --nocapture

# Run doc tests
cargo test --doc

# Check formatting
cargo fmt --check

# Auto-format code
cargo fmt

# Run clippy lints
cargo clippy
cargo clippy -- -D warnings

# Run all checks (fmt + clippy + test)
cargo check
```

### Project Structure
- **Main binary**: `src/bin/`
- **Core NES emulation**: `src/nes/` (cpu, ppu, apu, mapper, cartridge)
- **GUI**: `src/gui.rs` and `src/app.rs`
- **Tests**: Inline in `src/nes/mapper.rs` (16 tests)

### Testing Philosophy
- Tests are defined inline using `#[test]` in `src/nes/mapper.rs`
- Use helper functions like `patterned_banks()` and `make_cart()` for test setup
- Tests verify mapper behavior, IRQ timing, bank switching, etc.

## Code Style Guidelines

### General Principles
- Use Rust 2024 edition (already set in Cargo.toml)
- Prefer explicit error handling with `anyhow::Result`
- Keep functions focused and small
- Add documentation for public APIs

### Naming Conventions
- **Types/Structs**: `PascalCase` (e.g., `struct Nes`, `enum Mirroring`)
- **Functions/Variables**: `snake_case` (e.g., `fn step_cpu()`, `let total_cycles`)
- **Constants**: `SCREAMING_SNAKE_CASE` (e.g., `const FLAG_CARRY: u8 = 0x01`)
- **Modules**: `snake_case` (e.g., `mod nes`, `mod mapper`)
- **Visibility**: Use `pub(crate)` for module-internal public APIs

### Types
- Use fixed-width integers: `u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64`
- Use `usize` for array indices and lengths
- Prefer explicit type annotations for clarity
- Use `Option<T>` and `Result<T, E>` for optional/erroneous values

### Imports
```rust
// Group by: std → external → internal
use std::collections::VecDeque;
use anyhow::Result;
use super::{Flag, Type};
```

### Error Handling
- Use `anyhow::Result` for fallible operations (already a dependency)
- Use `bail!()` macro for early returns with errors
- Return appropriate error types from public APIs

### Formatting
- Use `cargo fmt` for automatic formatting
- Maximum line length: 100 characters (Rust default)
- Use 4 spaces for indentation
- No trailing whitespace
- One blank line between function definitions

### Control Flow
```rust
// Prefer early returns
if condition {
    return early;
}

// Use match for exhaustive patterns
match value {
    A => handle_a(),
    B => handle_b(),
    _ => handle_default(), // Don't forget wildcard!
}

// Avoid unnecessary braces
if condition {
    do_something()
}
```

### Documentation
- Document public functions with `///` doc comments
- Document modules with `//!` at file top
- Include examples in doc comments when helpful
- Keep comments focused on "why", not "what"

### Specific NES Emulator Patterns

#### Cycle Accuracy
- Implement `tick_cpu_cycle()` and `tick_ppu_cycle()` on mappers that need them
- Use `notify_ppu_read_addr()` and `notify_ppu_write_addr()` for PPU-mapper communication
- Track IRQ states properly with `irq_pending()` and `clear_irq()`

#### Memory Access
- Always use open bus behavior: read-modify-write operations should use the last written value
- Handle memory mirroring correctly (2KB RAM at 0x0000-0x07FF mirrors to 0x0800-0x1FFF)

#### PPU Registers
- PPU registers at 0x2000-0x3FFF mirror every 8 bytes
- Implement proper vblank/NMI timing
- Handle sprite 0 hit detection with cycle accuracy

## Common Patterns

### Mapper Implementation
```rust
pub trait Mapper {
    fn cpu_read(&mut self, addr: u16) -> u8;
    fn cpu_write(&mut self, addr: u16, value: u8);
    fn ppu_read(&mut self, addr: u16) -> u8;
    fn ppu_write(&mut self, addr: u16, value: u8);
    fn mirroring(&self) -> Mirroring;
    // Optional: tick_cpu_cycle, tick_ppu_cycle, irq_pending, etc.
}
```

### Test Helper Pattern
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn patterned_banks(total_size: usize, bank_size: usize) -> Vec<u8> { ... }
    fn make_cart(...) -> Cartridge { ... }

    #[test]
    fn my_test() {
        // Arrange
        let mut mapper = MapperX::new(make_cart(...));
        // Act
        mapper.cpu_write(0x8000, 0xFF);
        // Assert
        assert_eq!(mapper.cpu_read(0x8000), expected);
    }
}
```

## Useful Commands for Development

```bash
# Watch mode for development (requires cargo-watch)
cargo watch -x check -x test

# Run with specific test ROM
cargo run --release --bin cathode8 -- /path/to/rom.nes

# Run stress test
cargo run --release --bin stress_runner -- --rom <rom> --iterations 500

# Profile (requires flamegraph)
cargo flamegraph --bin cathode8
```

## Getting Help
- NESDev wiki: https://www.nesdev.org/wiki/
- NESDev forums: https://forums.nesdev.org/
- Check existing mappers in `src/nes/mapper.rs` for implementation examples
