# BrainFork JIT Compiler

BrainFork is a high-performance, ahead-of-time (AOT) and just-in-time (JIT) compiler for the Brainfuck esoteric language written in Rust. It targets the x86_64 architecture under the Microsoft x64 calling convention, compiling source instructions directly into native machine code. It bypasses abstract syntax tree (AST) interpretation by emitting raw opcodes directly into dynamically allocated executable memory pages.

## Key Features

* **Native x86_64 Execution**: Emits raw machine instructions directly to memory, achieving raw hardware execution speeds.
* **Run-Length Encoding (RLE)**: Automatically collapses repeated operations (like `+++++`) into single immediate-operand assembly instructions (`add byte ptr [rcx], 5`).
* **Idiom Matching Optimization**: Detects common programming patterns like clear loops (`[-]`) and compiles them directly into static zeroing instructions (`mov byte ptr [rcx], 0`).
* **Dynamic Memory Security Allocation**: Uses the Win32 API (`VirtualAlloc`/`VirtualProtect`) to strictly adhere to W^X (Write XOR Execute) memory security principles.
* **Lexical Scope Backpatching**: Employs a single-pass index tracking stack to accurately calculate relative jump offsets (`je`/`jmp`) for deeply nested loops.

## Technical Architecture

### Memory Layout and State Management

The system splits memory management into two distinct native regions using system allocations:

1. **Code Buffer**: An execution block allocated via `VirtualAlloc`. During the compilation phase, it is granted `PAGE_READWRITE` permissions. Once compilation is complete, it is locked down to `PAGE_EXECUTE_READ` before execution.
2. **Tape Buffer**: A continuous linear tape allocated with `PAGE_READWRITE` permissions.

### Application Binary Interface (ABI) Translation

The compiler leverages the Microsoft x64 calling convention (or `extern "win64"` ABI). Under this convention, the first integer or pointer argument passed to a function call is automatically assigned to the `RCX` register.

By passing the pointer to the tape buffer as the first argument to the JIT-compiled block, the address is loaded into `RCX`. All generated instructions use `RCX` as the base pointer for tape navigation.

### x86_64 Instruction Mapping

| Brainfuck | Optimization | x86_64 Assembly | Machine Code Bytes |
| --- | --- | --- | --- |
| `+` | Single | `inc byte ptr [rcx]` | `0xFE, 0x01` |
| `+` $\times N$ | Folded | `add byte ptr [rcx], N` | `0x80, 0x01, N` |
| `-` | Single | `dec byte ptr [rcx]` | `0xFE, 0x09` |
| `-` $\times N$ | Folded | `sub byte ptr [rcx], N` | `0x80, 0x29, N` |
| `>` $\times N$ | Folded | `add rcx, N` | `0x48, 0x83, 0xC1, N` |
| `<` $\times N$ | Folded | `sub rcx, N` | `0x48, 0x83, 0xE9, N` |
| `[-]` | Idiom Match | `mov byte ptr [rcx], 0` | `0xC6, 0x01, 0x00` |
| `[` | Loop Entry | `cmp byte ptr [rcx], 0`<br>

<br>`je <offset>` | `0x80, 0x39, 0x00`<br>

<br>`0x74, <offset>` |
| `]` | Loop Exit | `jmp <offset>` | `0x5E, <offset>` |
| `.` | Input/Output | Native C ABI Call | Preserves `RCX`, sets shadow space, calls `print_char` |

## Code Structure

* `BrainFork::new()`: Handles OS-level memory requests, allocating separate segments for the tape execution context and the generated code block.
* `BrainFork::compile()`: Tokenizes the input stream, measures instruction runs, handles lookaheads for clear loop identification, resolves Two's Complement relative jump distances, and commits code to memory.
* `BrainFork::execute()`: Uses a `transmute` cast to execute the allocated memory block as an `extern "win64" fn(*mut u8)`.

## Prerequisites

* **Operating System**: Windows 10 / 11 or Windows Server (x64 architectures).
* **Toolchain**: Rust 1.70.0 stable or higher with the `x86_64-pc-windows-msvc` target profile.

## Usage Example

```rust
fn main() {
    let mut Jit = BrainFork::new();

    // This program initializes cell 0 to 65 (ASCII 'A') using optimized runs 
    // and then calls the native stdout printing sequence.
    let source_code = "+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++.";

    jit.compile(source_code);
    
    // Executes the native x86_64 code block directly on the CPU
    jit.execute(); 
}

```

## Compilation and Testing

To compile the project with maximum release optimizations:

```bash
cargo build --release

```

To run unit validation suites checking loop offset mathematics and boundary overflows:

```bash
cargo test

```
