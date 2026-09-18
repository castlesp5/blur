# blur
## NOTE from castlesp5:  
 The Editor is currently under construction, if you found any bug please report it as soon as possible to fix it, V1.0 is gonna be out soon :)  
A minimal, vim-inspired terminal text editor written in Rust, built on top of [`ratatui`](https://github.com/ratatui-org/ratatui), [`crossterm`](https://github.com/crossterm-rs/crossterm), [`syntect`](https://github.com/trishume/syntect), and [`opaline`](https://github.com/hyperb1iss/opaline).

```
Blur 0.1.1
```

Made by [castlesp5](https://github.com/castlesp5), [Artem Tsitronov](https://github.com/artemtsitronov)

## Features

- **Modal editing** - Normal and Insert modes, with vim-style motions
- **Syntax highlighting** - automatic language detection by file extension via `syntect`, rendered with the `base16-mocha.dark` theme
- **Save / Open prompts** - inline command-line style file save and open, with error feedback in the footer
- **Fast paste** - bracketed paste support inserts pasted text in a single operation instead of one keystroke at a time
- **Horizontal & vertical scrolling** - the viewport follows the cursor without wrapping or corrupting long lines
- **Status bar** - current mode, file name, and cursor position (`x, y`)

## Installation

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain, 2024 edition)
- Cargo

### Build from source

```bash
$> git clone git@github.com:castlesp5/blur.git
$> cd blur
$> cargo build --release
```

or
```bash
$> ./blur <optional path file>
```
### Run

```bash
cargo run --release
# or, after building:
./target/release/blur [file]
```

If a file path is given as an argument, `blur` will open it (or create it in memory if it doesn't exist yet - it will be written to disk on save).

## Usage

### Modes

| Mode | Indicator | Description |
|---|---|---|
| Normal | `NORMAL` | Navigate and issue commands |
| Insert | `INSERT` | Type text directly into the buffer |
| Save   | `Save file into : ...` | Enter a destination path to save |
| Open   | `File to Open : ...`   | Enter a path to load a file |

### Normal mode keybindings

| Key | Action |
|---|---|
| `h` / `←` | Move cursor left |
| `l` / `→` | Move cursor right |
| `j` / `↓` | Move cursor down |
| `k` / `↑` | Move cursor up |
| `a` | Append - move one column right and enter Insert mode |
| `o` | Open a new line below the current line and enter Insert mode |
| `e` | Move to the start of the next word |
| `b` | Move to the end of the previous word |
| `w` | Save to the current file (prompts for a path if none is set) |
| `W` | Save as - always prompts for a destination path |
| `O` | Open a file |
| `q` | Quit |

### Insert mode

| Key | Action |
|---|---|
| Any character | Insert at cursor |
| `Enter` | Insert newline |
| `Tab` | Insert 4 spaces |
| `Backspace` | Delete character before cursor |
| `Esc` | Return to Normal mode |
| `←` `→` `↑` `↓` | Move cursor (does not insert) |

### Save / Open mode

| Key | Action |
|---|---|
| Any character | Append to the path being typed |
| `Backspace` | Remove last character from the path |
| `Enter` | Confirm - save or open the file |
| `Esc` | Cancel and return to Normal mode |

### Pasting

`blur` enables terminal bracketed paste mode on startup. Pasting (e.g. `Ctrl+V` or your terminal's paste shortcut) inserts the entire clipboard contents in one operation, so large pastes appear instantly rather than character-by-character. Windows-style (`\r\n`) and legacy Mac-style (`\r`) line endings are normalized to `\n` on insert.

## Project structure

```
src/
├── main.rs      # entry point, event loop, and rendering
├── modes.rs     # normal / insert / save / open mode handlers
├── controls.rs  # shared cursor movement logic
└── helpers.rs   # Tab (buffer state) and Highlighter (syntax highlighting)
```

### Architecture notes

- **`Tab`** (`helpers.rs`) holds all editor state for a single buffer: the file name, raw text (`input_box`), cursor position (`cursor_x`, `cursor_y`), a flattened byte offset into the buffer (`gcursor`), and scroll offsets.
- **`gcursor`** is the single source of truth for *where* an edit happens in the underlying `String`. Every motion or edit that changes `cursor_x`/`cursor_y` must keep `gcursor` in sync, or insertions/deletions will land at the wrong byte offset.
- **`Highlighter`** wraps `syntect`, detecting syntax from the file extension and falling back to plain text for buffers with no file name or unrecognized extensions.
- The **status bar** is split into multiple regions: current mode, file name, cursor position, and editor info.

## Known limitations
- Word-motion commands (`e`,  `b`) currently operate within the current line only and do not wrap across line boundaries.
- Only a single buffer/tab is supported at this time.

## License

Licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)


## Contributing

Issues and pull requests are welcome. Please keep cursor/`gcursor` invariants in mind when touching motion or editing code - see **Architecture notes** above.
