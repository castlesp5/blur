# Blur — V0.9

A fast, modal, terminal-based text editor written in Rust — built on [`ratatui`](https://github.com/ratatui-org/ratatui) and [`crossterm`](https://github.com/crossterm-rs/crossterm), with Vim-inspired keybindings, syntax highlighting via `syntect`, and theming powered by `opaline`.

> **Status:** early / actively developed. Keybindings and internals are still evolving.

---

## Table of Contents

- [Features](#features)
- [Installation](#installation)
  - [Prerequisites](#prerequisites)
  - [Build from source](#build-from-source)
- [Usage](#usage)
- [Modes](#modes)
  - [Normal Mode](#normal-mode)
  - [Insert Mode](#insert-mode)
  - [Select Mode (char-wise)](#select-mode-char-wise)
  - [Select-Line Mode](#select-line-mode)
  - [Save / Open Prompts](#save--open-prompts)
  - [Unsaved Work Prompt](#unsaved-work-prompt)
- [Tabs](#tabs)
- [Undo / Redo](#undo--redo)
- [Syntax Highlighting & Theming](#syntax-highlighting--theming)
- [Project Structure](#project-structure)
- [Architecture Notes](#architecture-notes)
- [Known Limitations](#known-limitations)
- [Roadmap](#roadmap)
- [Contributing](#contributing)
- [License](#license)

---

## Features

- **Modal editing** — Normal, Insert, Select (character-wise), and Select-Line modes, in the spirit of Vim.
- **Multiple tabs** — open and switch between several files/buffers in one session.
- **Syntax highlighting** — powered by `syntect`, themed through `opaline` (defaults to `catppuccin-mocha`).
- **Undo / redo** — a dedicated edit-record stack tracks fine-grained operations (character inserts/deletes, line splits/merges, indentation, line moves, etc.) for reliable, reversible edits.
- **Bracketed paste support** — multi-line pastes are inserted correctly in Insert mode.
- **Prompt-driven save/open** — save to a new path or open another file without leaving the editor.
- **Indentation controls** — indent/unindent single lines or whole selections.
- **Unsaved-work protection** — warns before quitting a tab (or the app) with unsaved changes.
- **Powerline-style status bar** — shows current mode, file name, save state, tab index, and cursor position.

## Installation

### Prerequisites

- [Rust & Cargo](https://www.rust-lang.org/tools/install) (stable toolchain)
- A terminal emulator with reasonable UTF-8 / true-color support

### Build from source

```bash
git clone git@github.com:castlesp5/blur.git
cd blur
cargo build --release
```

The compiled binary will be available at `target/release/blur`.

## Usage

Open the editor with no file (a fresh, untitled buffer):

```bash
$>./blur
```

Open a specific file:

```bash
$>./blur path/to/file.rs
```

If the file doesn't exist yet, Blur will start with an empty buffer bound to that path — write with `w` to create it.

## Modes

Blur is modal: keys behave differently depending on the current mode, shown in the status bar (bottom-left).

### Normal Mode

The default mode for navigation and commands.

| Key(s)         | Action                                             |
|----------------|-----------------------------------------------------|
| `h` `j` `k` `l` / Arrow keys | Move cursor left / down / up / right   |
| `g`            | Jump to the start of the file                       |
| `G`            | Jump to the end of the file                         |
| `e`            | Move to the end of the next word                    |
| `b`            | Move to the start of the previous word              |
| `i`            | Enter Insert mode at the cursor                      |
| `a`            | Enter Insert mode, appending after the cursor        |
| `o`            | Insert a new line below and enter Insert mode        |
| `J`            | Move current line down                               |
| `K`            | Move current line up                                 |
| `>`            | Indent current line                                  |
| `<`            | Unindent current line                                |
| `d`            | Delete current line                                  |
| `u`            | Undo                                                  |
| `r`            | Redo                                                  |
| `v`            | Enter Select mode (character-wise)                    |
| `V`            | Enter Select-Line mode                                |
| `w`            | Save current file (prompts for a path if the buffer is unnamed) |
| `W`            | Save as (always prompts for a path)                    |
| `O`            | Open a file (prompts for a path)                      |
| `N`            | Open a new, empty tab                                 |
| `Tab` / `n`    | Switch to the next tab                                |
| `Shift+Tab`    | Switch to the previous tab                            |
| `Delete`       | Delete the character under the cursor                 |
| `Backspace`    | Delete the character before the cursor                |
| `q`            | Close the current tab / quit (prompts if unsaved)      |

> **Note:** `w` is overloaded — it saves the file when pressed as a plain command in Normal mode. Use `W` if you always want to be prompted for a new path.

### Insert Mode

Free text entry, entered via `i`, `a`, or `o` from Normal mode.

| Key(s)      | Action                                  |
|-------------|-------------------------------------------|
| Any character | Insert at the cursor                    |
| `Enter`     | Split the line at the cursor               |
| `Tab`       | Insert 4 spaces                            |
| `Backspace` | Delete previous character / merge with previous line |
| `Delete`    | Delete character under cursor / remove empty line |
| Arrow keys  | Move the cursor without leaving Insert mode |
| `Esc`       | Return to Normal mode                       |

Consecutive typed characters are batched into a single undo step, so `u` after typing a word undoes the whole word at once rather than one character at a time.

### Select Mode (char-wise)

Entered with `v` from Normal mode. Selects a character-wise range on the current line only, anchored where `v` was pressed.

| Key(s)     | Action                              |
|------------|----------------------------------------|
| Movement keys | Extend/move the selection endpoint  |
| `d`        | Delete the selected text, return to Normal mode |
| `Esc`      | Cancel and return to Normal mode        |

### Select-Line Mode

Entered with `V` from Normal mode. Selects whole lines between the anchor row and the cursor row.

| Key(s) | Action                                             |
|--------|------------------------------------------------------|
| `g`    | Jump to the first line (adjusts selection)            |
| `G`    | Jump to the last line (adjusts selection)             |
| `J`    | Move the selected block of lines down                 |
| `K`    | Move the selected block of lines up                   |
| `>`    | Indent all selected lines                             |
| `<`    | Unindent all selected lines                           |
| `d`    | Delete all selected lines, return to Normal mode       |
| `Esc`  | Cancel and return to Normal mode                        |

### Save / Open Prompts

Triggered by `W` (save as) or `O` (open) in Normal mode, or automatically by `w` when the buffer has no associated file yet.

| Key(s)      | Action                          |
|-------------|-----------------------------------|
| Characters  | Type the target file path          |
| `Backspace` | Delete the last character          |
| `Enter`     | Confirm and save/open               |
| `Esc`       | Cancel and return to Normal mode    |

If a save or open fails (e.g. invalid path or permissions), the status bar reports `Can't save file` / `Can't open file`.

### Unsaved Work Prompt

Shown when closing a tab or quitting with unsaved changes.

| Key   | Action                                   |
|-------|---------------------------------------------|
| `y`   | Discard changes and close/quit               |
| Any other key | Cancel and return to Normal mode      |

## Tabs

Blur supports multiple open buffers ("tabs") in a single session:

- `N` opens a new, empty tab and switches to it.
- `Tab` / `n` and `Shift+Tab` cycle forward/backward through open tabs (wrapping around at the ends).
- The status bar shows the active file name (prefixed with `*` if unsaved) and its tab index.
- Closing the last remaining tab quits the application.

## Undo / Redo

Every meaningful edit — character insertions/deletions, string insert/removal (e.g. indentation), line splits/merges, line insertions/removals, and whole-line moves — is recorded as an `EditRecord`. `u` pops from the undo stack and applies the inverse operation; `r` pops from the redo stack and re-applies it. Performing a new edit clears the redo stack, matching standard editor semantics.

## Syntax Highlighting & Theming

- Highlighting is powered by [`syntect`](https://github.com/trishume/syntect), using its bundled default syntax definitions.
- Colors come from an [`opaline`](https://crates.io/crates/opaline) theme (`catppuccin-mocha` by default), adapted into a `syntect` theme.
- Files without a recognized extension/name fall back to plain, unstyled text.
- Foreground/background contrast for UI chrome (status bar segments) is computed at runtime using relative luminance and WCAG contrast ratios, so status bar text stays readable against any theme's accent colors.

## Project Structure

```
src/
├── main.rs           # Entry point, event loop, top-level rendering (status bar, layout, cursor)
├── controls.rs        # Shared cursor-movement primitives (arrow keys, hjkl)
├── normal_mode.rs      # Normal-mode command handling
├── modes.rs            # Insert mode, save/open prompts, unsaved-work prompt, paste handling
├── select_modes.rs     # Select (char-wise) and Select-Line mode handling
└── helpers.rs           # Tab, Visual, Highlighter, EditRecord, undo/redo application, logging
```

## Architecture Notes

- **Event loop:** `main.rs` owns a `Vec<Tab>` plus a `mode: i32` state machine (`0` = Normal, `1` = Insert, `2` = Select, `3` = Select-Line, `10`/`11` = Save/Open prompts, `403` = unsaved-work prompt, `401`/`402` = error states). Each keypress is dispatched to the handler for the current mode.
- **`Tab`** holds per-buffer state: `input_box` (the lines of text), cursor position, scroll offsets, save state, and independent undo/redo stacks — so undo history and viewport are per-tab.
- **`Visual`** tracks the anchor point (`v_x`, `v_y`) for Select and Select-Line modes.
- **`EditRecord`** is an enum of reversible operations; `apply_inverse` and `apply_forward` in `helpers.rs` are the single source of truth for how each operation is undone/redone, keeping edit logic and undo logic in sync.
- **Rendering:** `renderer()` in `main.rs` lays out the buffer viewport and a two-segment status bar, auto-scrolls to keep the cursor in view (accounting for Unicode display width), and positions the terminal cursor precisely — including inside the save/open command line.

## Known Limitations

- Select mode (`v`) only supports selections within a single line.
- Word-motion commands (`e`, `b`) are basic space-delimited jumps rather than full Vim word-object semantics.
- No search/replace, no line numbers, no split panes yet.
- Debug logging (`helpers::log`) writes to `blur-log.txt` in the working directory — remember to clean this up or gate it behind a debug flag before distributing builds.

## Roadmap

- [ ] Search and search-and-replace
- [ ] Line numbers / relative line numbers
- [ ] Multi-line visual selection
- [ ] Configurable keybindings and theme selection at runtime
- [ ] Split panes / window management
- [ ] Config file support (e.g. `~/.config/blur/config.toml`)

## Contributing

Issues and pull requests are welcome. If you're adding a new command, please:

1. Add the corresponding `EditRecord` variant (and its `apply_forward`/`apply_inverse` handling) if the change should be undoable.
2. Keep mode-specific logic in its own module (`normal_mode.rs`, `modes.rs`, `select_modes.rs`) rather than in `main.rs`.
3. Update the keybinding tables in this README.

## License  

This project is dual-licensed under either of:

    MIT License
    Apache License, Version 2.0


## Author
`Ismael Boujdad` [www.github.com/castlesp5]  
  
### Credits
`Artem Tsitronov` [https://github.com/artemtsitronov]


