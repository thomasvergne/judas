# Judas HDL - VS Code Extension

Official Visual Studio Code extension for **Judas HDL**, providing rich language features, diagnostics, autocompletion, hover documentation, and jump-to-definition powered by `judas-lsp`.

## Features

- **Real-Time Diagnostics**: Instant feedback on syntax errors and typecheck errors (width mismatches, undefined signals with suggestions, clock trigger checks).
- **Hover Information**: Shows module signatures, port types, register definitions, packed struct bit layouts, interface protocols, and built-in type/function documentation.
- **Go to Definition**: Jump directly to module, port, register, let wire, struct, enum, and interface declarations.
- **Intelligent Autocompletion**:
  - Keywords, hardware primitives, and built-in types (`UInt<W>`, `Bits<W>`, `Clock`, `Bool`).
  - Signal completions and hardware function snippets (`extend`, `truncate`, `concat`, `repeat`, `slice`).
  - Member access: type `instance.` to complete ports of an instantiated module!
  - Path access: type `Enum::` or `namespace::` to complete variants and declarations.
- **Document Symbols / Outline**: Hierarchical overview of modules, ports, registers, signals, structs, and enums in the editor outline and breadcrumbs.
- **Syntax Highlighting**: Comprehensive TextMate grammar for Judas HDL.

## Installation

### 1. Build or Install Judas LSP

From the Judas repository root:

```bash
# Option A: Install judas-lsp to your PATH
cargo install --path crates/judas-lsp

# Option B: Build judas-lsp in target/release
cargo build --release -p judas-lsp
```

### 2. Install the Extension in VS Code

You can link or copy the `editors/vscode` folder to your VS Code extensions directory:

```bash
# On macOS / Linux
ln -s "$(pwd)/editors/vscode" ~/.vscode/extensions/judas-hdl

# Or on Windows (PowerShell)
New-Item -ItemType SymbolicLink -Path "$HOME/.vscode/extensions/judas-hdl" -Target "$PWD/editors/vscode"
```

Inside `editors/vscode`:
```bash
npm install
```

Then reload VS Code (`Developer: Reload Window` in the command palette). Any `.jd` file will automatically be recognized and connected to the language server!

## Configuration

In your VS Code `settings.json`:

```json
{
  "judas.lsp.path": "judas-lsp",
  "judas.trace.server": "messages"
}
```
