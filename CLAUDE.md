# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Edit is a simple terminal text editor inspired by MS-DOS Editor, built in Rust with a focus on minimal binary size and high performance. The project uses nightly Rust features and optimizes heavily for speed and size.

## Build Commands

### Standard Development Build
```bash
cargo build
```

### Release Build
Choose based on Rust version:
- **Rust 1.90 or earlier:**
  ```bash
  cargo build --config .cargo/release.toml --release
  ```
- **Rust 1.91+:**
  ```bash
  cargo build --config .cargo/release-nightly.toml --release
  ```

### Testing
```bash
# Standard tests
cargo test

# ICU-related tests (requires ICU library configuration)
cargo test -- --ignored
```

### Running the Editor
```bash
cargo run
cargo run -- <file_path>  # Open specific file
```

### Code Formatting
```bash
cargo fmt
```

## Build Configuration

The project requires **nightly Rust** (see `rust-toolchain.toml`). The codebase uses unstable features:
- `allocator_api`
- `linked_list_cursors`
- `breakpoint`
- `cold_path`
- And others (see `src/lib.rs`)

### Environment Variables for Build

- **`EDIT_CFG_ICUUC_SONAME`**: ICU Unicode library name (e.g., `libicuuc.so.76`)
- **`EDIT_CFG_ICUI18N_SONAME`**: ICU i18n library name (e.g., `libicui18n.so.76`)
- **`EDIT_CFG_ICU_CPP_EXPORTS`**: Set to `true` for C++ symbols (default on macOS)
- **`EDIT_CFG_ICU_RENAMING_VERSION`**: ICU version number for versioned symbols (e.g., `76`)
- **`EDIT_CFG_ICU_RENAMING_AUTO_DETECT`**: Set to `true` for runtime version detection
- **`EDIT_CFG_LANGUAGES`**: Comma-separated list of languages to include (see `i18n/edit.toml`)

## Architecture

This project has unique architectural decisions focused on performance:

### Text Buffer (`src/buffer/`)
- **No line break tracking**: The buffer doesn't maintain line positions as state
- **O(n) seeking**: Finding a line requires scanning through the document for line breaks
- Relies on extremely fast SIMD operations to make this viable
- Current cursor position is the primary tracked state

### Performance-Critical Modules

1. **SIMD (`src/simd/`)**:
   - Custom `memchr2` implementations for finding line breaks at >100GB/s
   - `memset` optimizations
   - Forward/backward line scanning

2. **Unicode (`src/unicode/`)**:
   - `Utf8Chars` iterator with transparent U+FFFD replacement (~4GB/s)
   - Grapheme cluster segmentation via `MeasurementConfig` (~600MB/s)
   - Makes word-wrap fast enough for large files

3. **Framebuffer (`src/framebuffer.rs`)**:
   - Game-like intermediate rendering buffer
   - Accumulates changes, handles color blending
   - Diffs against previous frame to minimize terminal output

4. **Immediate Mode UI (`src/tui.rs`)**:
   - Similar to ImGui design
   - DOM-like tree structure rebuilt each frame
   - Nodes identified by hashed classnames
   - See module docs for detailed algorithm explanation

### Platform Abstractions (`src/sys/`)
- `sys/unix.rs`: Unix-specific terminal handling
- `sys/windows.rs`: Windows console API handling
- Platform init happens first in execution order

### VT Parser (`src/vt.rs`)
- Handles terminal escape sequences
- Terminal setup in `src/bin/edit/main.rs::setup_terminal()`

### Application Entry (`src/bin/edit/`)
- ~90% UI code and business logic
- Modules:
  - `main.rs`: Entry point, terminal setup
  - `state.rs`: Application state
  - `documents.rs`: Document management
  - `draw_*.rs`: UI rendering for different components
  - `localization.rs`: i18n support

### Memory Management (`src/arena/`)
- Custom arena allocator
- `scratch_arena()` for temporary allocations
- Initialized early with platform-specific capacity (128MB on 32-bit, 512MB on 64-bit)

## Code Style

- **Imports**: Grouped as `StdExternalCrate`, granularity at module level
- **Formatting**: Uses Rust 2024 edition style (`rustfmt.toml`)
- **Unix newlines**: Enforced
- **Field init shorthand**: Preferred

## Performance Priorities

1. **Good performance**: Fast enough for 1GB+ files
   - SIMD optimizations critical
   - O(n) navigation acceptable due to SIMD speed
2. **Binary size**: This fork is less concerned with binary size compared to upstream
   - Adding desirable features is prioritized over minimal binary size
   - Good optimization practices are still important
   - Dependencies can be added when they provide clear value

## Localization

Translations are in `i18n/edit.toml`. The build process (`build/i18n.rs`) generates localization code at compile time.

## Terminal-Related Issues

If debugging terminal behavior, investigate:
1. VT parser in `src/vt.rs`
2. Platform code in `src/sys/`
3. `setup_terminal()` in `src/bin/edit/main.rs`

## Feature Development

- Desirable features are prioritized over binary size concerns
- Good optimization practices should still be maintained
- Plugin support is planned for future extensibility

## Git Workflow

- **Never push to remote unless explicitly requested by the user**
- When committing, only add, commit, and verify - do not push
- Always verify commits with `git status` after committing
