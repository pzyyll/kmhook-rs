# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

KMHook is a cross-platform keyboard and mouse event listening library implemented in Rust, used for listening to global keyboard and mouse events, registering global hotkeys, and sending simulated keyboard input.

## Common Commands

Since Cargo is not installed on your system, you need to first ensure that the Rust environment is properly set up. The following are standard Rust project commands:

```bash
# Build the project
cargo build

# Build the release version
cargo build --release

# Run tests
cargo test

# Run examples
cargo run --example shortcut
cargo run --example i_test

# Check code (without building)
cargo check

# Format code
cargo fmt

# Run clippy checks
cargo clippy
```

## Core Architecture

### Module Structure

- `src/lib.rs` - Library entry point; imports appropriate listeners according to the target platform
- `src/enginer.rs` - Provides the global API, wrapping lower-level listener functionality
- `src/types.rs` - Defines core types: EventType, KeyInfo, MouseInfo, Shortcut, etc.
- `src/consts.rs` - Constant definitions
- `src/utils.rs` - Utility functions

### Platform-specific Implementations

- `src/windows/` - Windows platform implementation
- `listener.rs` - Main implementation of the Windows event listener
- `event_loop.rs` - Windows message loop (currently using a fake version)
- `worker.rs` - Worker thread management
- `types_ext.rs` - Windows-specific type extensions
- `src/linux/` - Linux platform implementation (pending development)
- `src/macos/` - macOS platform implementation (pending development)

### Core Concepts

1.**EventListener Trait**: Defines the basic interface for event listeners

- `add_global_shortcut` - Add global hotkey
- `add_global_shortcut_trigger` - Add hotkey with support for repeated triggering
- `add_event_listener` - Add event listener
- `startup/shutdown` - Start/stop listener

2.**Shortcut System**:

- Supports modifier + regular key combinations
- Smart key name mapping (Ctrl->Control, Win->Meta, Cmd->Meta)
- Differentiates left/right keys (ControlLeft vs Control)
- String parsing ("Ctrl+Alt+T")

3.**Event Types**:

- `KeyboardEvent` - Keyboard event, includes KeyInfo
- `MouseEvent` - Mouse event, includes MouseInfo
- `All` - All event types

### Key Features

- **Global Listening**: Uses platform-specific low-level hooks to monitor system-level events
- **Thread Safety**: Ensures thread safety with Arc<Mutex<>>
- **Hotkey Triggering**: Supports both single and repeated trigger modes
- **Cross-platform Design**: Supports multiple platforms via conditional compilation

## Development Notes

- Currently, the main implementation is for Windows; the Linux/macOS modules are empty
- Uses external keycode library for key code mapping
- Windows implementation relies on the windows crate for system calls
- Event loop currently uses a fake implementation (`event_loop_fake.rs`)
- Test files are located in the `tests/` directory and as unit tests in `src/types.rs`

## Dependencies

- `keycode` - Key code mapping library (from Git)
- `windows` - Windows API bindings (Windows only)
- `bitflags` - Bitflag support
- `lazy_static` - Static variable initialization
