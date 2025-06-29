# GEMINI.md

This file provides instructions and context for interacting with the `kmhook-rs` project.

## About the Project

`kmhook-rs` is a Rust library for creating cross-platform global keyboard and mouse event listeners (hooks). It aims to provide a simple, lightweight, and easy-to-use API for capturing user input events system-wide.

The core of the library is the `Listener` struct, which manages event callbacks and shortcut handlers. It uses a dedicated event loop and worker threads to process raw input from the operating system and dispatch events accordingly.

## Key Files

-   `src/lib.rs`: The main library entry point, which exposes the public API.
-   `src/enginer.rs`: Provides a simplified, high-level API over the `Listener` for common use cases.
-   `src/types.rs`: Defines the core data structures and enums used throughout the library, such as `EventType`, `KeyId`, and `Shortcut`.
-   `src/windows/listener.rs`: The Windows-specific implementation of the `Listener`. This is where the core logic for handling Windows hooks resides.
-   `src/windows/event_loop.rs`: Manages the Windows message loop required for receiving system events.
-   `examples/`: Contains usage examples that are helpful for understanding how to use the library.

## Development Guidelines

### Code Style

-   **Match Existing Style**: Adhere to the existing code style, formatting, and naming conventions. The project uses standard Rust formatting (`rustfmt`).
-   **Comments**: Add comments to explain *why* a piece of code is necessary, especially for complex or non-obvious logic. Avoid explaining *what* the code does, as that should be clear from the code itself.
-   **Error Handling**: Use `Result<T, E>` for functions that can fail. Use `?` to propagate errors where appropriate.

### Making Changes

1.  **Understand the Goal**: Before writing any code, make sure you understand the requirements of the task.
2.  **Identify Relevant Code**: Use the file descriptions above to locate the relevant parts of the codebase.
3.  **Write Small, Focused Changes**: Make small, incremental changes. This makes it easier to review and debug.
4.  **Test Your Changes**: If you are adding a new feature or fixing a bug, add or update tests to cover your changes. Run the tests using `cargo test`.
5.  **Update Examples**: If your changes affect the public API, update the examples in the `examples/` directory to reflect the new usage.

### Committing and Pull Requests

-   Write clear and concise commit messages.
-   A good commit message should explain the "what" and the "why" of the change.
-   Ensure all tests pass before submitting a pull request.

## How to Run Examples

To run an example, use the following command:

```sh
cargo run --example <example_name>
```

For instance, to run the `listener` example:

```sh
cargo run --example listener
```
