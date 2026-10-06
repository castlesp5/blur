# blur 0.10

A fast, modal terminal text editor written in Rust. Vim-inspired keybindings, automatic syntax highlighting, and a transparent Catppuccin Mocha interface built on [ratatui](https://github.com/ratatui-org/ratatui) and [crossterm](https://github.com/crossterm-rs/crossterm).

## Features

- Modal editing: Normal, Insert, Select, and Select-Line modes
- Multiple tabs with per-tab undo history and viewport
- Automatic language detection: file path, shebang, and content sniffing via `syntect`
- Auto-indentation on Enter, with opener-aware extra indent
- Fine-grained undo/redo across inserts, deletes, splits, merges, indents, and line moves
- Bracketed paste support in Insert mode and in save/open prompts
- Prompt-driven save, save-as, and open with retry on failure
- Unsaved-work confirmation across all open tabs
- Transparent UI: rounded mode-colored frame, pill tab bar and statusline, Nerd Font icons
- Theme picker (`t`): all 39 built-in opaline themes with live preview and accent dots
- IDE-grade indentation: opener-aware indent, electric closers, bracket-split Enter

## Requirements

- Stable Rust toolchain ([install](https://www.rust-lang.org/tools/install))
- A terminal with UTF-8 and true-color support
- A Nerd Font for file and mode icons ([nerdfonts.com](https://www.nerdfonts.com))

## Install

Build from source:

```bash
git clone git@github.com:castlesp5/blur.git
cd blur
cargo build --release
```

The binary is produced at `target/release/blur`. Run `cargo clippy --all-targets` for lint checks.

## Usage

Open an empty buffer:

```bash
./blur
```

Open a file:

```bash
./blur path/to/file.rs
```

If the file does not exist, blur starts with an empty buffer bound to that path. Press `w` to write it.

## Keybindings

### Normal mode

| Keys | Action |
| ---- | ------ |
| `h j k l`, arrows | Move cursor |
| `g` / `G` | First line / last line |
| `e` / `b` | Next word / previous word |
| `E` / `B` | End of line / start of line |
| `i` / `a` | Insert at cursor / after cursor |
| `o` | New line below, enter Insert mode |
| `J` / `K` | Move current line down / up |
| `>` / `<` | Indent / unindent current line |
| `d` | Delete current line |
| `u` / `r` | Undo / redo |
| `v` / `V` | Select mode / Select-Line mode |
| `w` | Save (prompts for a path when unnamed) |
| `W` | Save as (always prompts) |
| `O` | Open file (prompts) |
| `N` | New empty tab |
| `Tab` / `n`, `Shift+Tab` | Next / previous tab |
| `Delete` / `Backspace` | Delete under / before cursor |
| `q` | Close tab or quit (confirms when any tab is unsaved) |
| `t` | Theme picker (live preview, `Enter` applies, `Esc` restores) |

### Insert mode

| Keys | Action |
| ---- | ------ |
| Text | Insert at cursor |
| `Enter` | Split line, carrying indentation (between brackets opens an indented middle line) |
| `Tab` | Insert 4 spaces |
| `Shift+Tab` | Remove one indent level |
| `}` `)` `]` | Electric dedent when typed on blank indentation |
| `Backspace` | Delete before cursor, or merge with previous line |
| `Delete` | Delete under cursor, or remove empty line |
| Arrows | Move without leaving Insert mode |
| `Esc` | Back to Normal mode |

Typed runs are batched into one undo step.

### Select mode (`v`)

Single-line selection anchored where `v` was pressed.

| Keys | Action |
| ---- | ------ |
| Movement keys | Extend selection |
| `>` / `<` | Indent / unindent line, back to Normal mode |
| `d` | Delete selection, back to Normal mode |
| `Esc` | Cancel |

### Select-Line mode (`V`)

Whole-line selection between the anchor and the cursor.

| Keys | Action |
| ---- | ------ |
| `g` / `G` | Extend to first / last line |
| `J` / `K` | Move selected block down / up |
| `>` / `<` | Indent / unindent selected lines |
| `d` | Delete selected lines, back to Normal mode |
| `Esc` | Cancel |

### Save, open, and error prompts

| Keys | Action |
| ---- | ------ |
| Text | Edit the file path |
| `Backspace` | Delete last character |
| `Enter` | Confirm |
| `Esc` | Cancel |

A failed save or open keeps the buffer and the typed path. Any key returns to the prompt for a retry.

### Unsaved-work prompt

Shown when quitting with unsaved changes in any tab.

| Keys | Action |
| ---- | ------ |
| `y` | Discard and close |
| Any other key | Cancel |

## Tabs

Tabs show file icons, numbers, and dirty dots. Duplicate file names gain parent-path context until unique, long names shorten in the middle, and the bar scrolls with `‹` `›` markers to keep the active tab visible.

## Themes

Press `t` in Normal mode for the theme picker: `j`/`k` or arrows move with live preview, `Enter` keeps the theme, `Esc` restores the previous one. Each row shows three accent dots sampled from the theme. No configuration or network involved.

## Configuration

Settings persist in `$XDG_CONFIG_HOME/blur/config.toml` (or `~/.config/blur/config.toml`):

```toml
theme = "dracula"
```

The file is written whenever a theme is applied and read at startup. Unknown theme names fall back to `catppuccin-mocha`. Only `theme` is stored for now.

## Undo and redo

Every edit is recorded as an `EditRecord` with an exact inverse. `u` applies the inverse, `r` re-applies it. Any new edit clears the redo stack. Undo history is per tab.

## Syntax highlighting

Highlighting uses `syntect` with the `catppuccin-mocha` theme loaded through `opaline`:

1. File path lookup (extension and file name).
2. Shebang and first-line detection (works for untitled buffers).
3. Content sniffing for common constructs.
4. Plain text fallback.

Results are cached per tab and invalidated on every edit, undo, and redo. Statusline contrast is derived at runtime from WCAG relative luminance.

## Project structure

```text
src/
  main.rs          Entry point, event loop, renderer
  controls.rs      Cursor movement primitives (arrows, hjkl)
  normal_mode.rs   Normal-mode commands
  modes.rs         Insert mode, save/open prompts, paste, auto-indent
  select_modes.rs  Select and Select-Line handling
  helpers.rs       Tab, Visual, Highlighter, EditRecord, undo/redo
```

## Architecture

- `main.rs` owns the tab list, the active index, and a numeric mode state machine. Each key event is dispatched to the handler for the current mode.
- `Tab` holds buffer lines, cursor, scroll offsets, saved flag, highlight cache, and independent undo/redo stacks.
- `Visual` holds the selection anchor for Select modes.
- `apply_inverse` and `apply_forward` in `helpers.rs` are the single implementation of every reversible operation.
- `renderer()` draws the tab bar, the framed viewport with gutter and scrollbar, and the statusline, then places the terminal cursor exactly, including inside prompts.

## Known limitations

- Select mode (`v`) is limited to a single line.
- Word motions are space-delimited and do not implement full Vim word objects.
- No search/replace or split panes yet.

## Roadmap

- [ ] Search and search-and-replace
- [ ] Multi-line character-wise selection
- [ ] Configurable keybindings and theme selection
- [ ] Split panes
- [ ] Config file support (for example `~/.config/blur/config.toml`)

## Contributing

Issues and pull requests are welcome.

1. Add an `EditRecord` variant with `apply_forward`/`apply_inverse` handling for any undoable change.
2. Keep mode logic in its module rather than in `main.rs`.
3. Update the keybinding tables above.
4. Keep `cargo clippy --all-targets` warning-free.

## License

Dual-licensed under either of:

- MIT License
- Apache License, Version 2.0

## Author

Ismael Boujdad ([github.com/castlesp5](https://github.com/castlesp5))

### Credits

Artem Tsitronov ([github.com/artemtsitronov](https://github.com/artemtsitronov))
