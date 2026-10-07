# blur

A modal terminal text editor in Rust. Vim-style keys, syntax highlighting, live markdown preview, and a transparent themed interface.

Built on [ratatui](https://github.com/ratatui-org/ratatui), [crossterm](https://github.com/crossterm-rs/crossterm), [syntect](https://github.com/trishume/syntect), and [opaline](https://crates.io/crates/opaline).

## Features

- Modal editing: normal, insert, visual, and visual-line modes
- Multiple tabs, each with its own undo history and viewport
- Automatic language detection, including shebangs and untitled buffers
- Smart indentation, electric brackets, bracket-split enter, undo of each
- Markdown preview pane with inline images
- 39 built-in themes with live preview
- Full mouse support
- Bracketed paste

## Requirements

- Stable Rust toolchain
- A terminal with UTF-8 and true color
- A Nerd Font for icons ([nerdfonts.com](https://www.nerdfonts.com))

## Build

```bash
git clone https://github.com/programmersd21/blur_ide.git
cd blur_ide
cargo build --release
```

## Usage

```bash
./blur              # empty buffer
./blur path/to/file # open a file in a new tab
```

## Keys

### Normal mode

| Keys | Action |
| ---- | ------ |
| `h j k l`, arrows | Move cursor |
| `g` / `G` | First / last line |
| `e` / `b` | Word forward / back |
| `E` / `B` | End / start of line |
| `i` / `a` / `o` | Insert, append, open line below |
| `J` / `K` | Move line down / up |
| `>` / `<` | Indent / unindent line |
| `d` | Delete line |
| `u` / `r` | Undo / redo |
| `v` / `V` | Visual / visual-line mode |
| `w` / `W` | Save / save as |
| `O` | Open file in a new tab |
| `N` | New empty tab |
| `Tab` / `Shift+Tab` | Next / previous tab |
| `t` | Theme picker |
| `P` | Toggle markdown preview |
| `X` | Close all saved tabs |
| `q` | Close tab or quit |
| `Q` | Quit everything |

### Insert mode

| Keys | Action |
| ---- | ------ |
| `Enter` | Split line with matching indent |
| `Tab` / `Shift+Tab` | Indent / unindent |
| `}` `)` `]` | Dedent when on blank indentation |
| `Backspace` | Delete before cursor, or merge lines |
| `Esc` | Back to normal mode |

### Other

| Keys | Action |
| ---- | ------ |
| `j` / `k`, arrows | Theme picker navigation |
| `Enter` | Apply theme |
| `Esc` | Restore previous theme |

## Mouse

- Click to place the cursor, drag to select
- Click a tab to switch, click a theme row to apply
- Click the left half of the confirm dialog to quit
- Scroll to scroll either pane

## Configuration

`$XDG_CONFIG_HOME/blur/config.toml`:

```toml
theme = "catppuccin-mocha"
```

Written when a theme is applied, read at startup.

## Markdown preview

Press `P` with a markdown file open. Headings, lists, quotes, code, and links render live as you type.

Images display through the kitty graphics protocol on kitty, WezTerm, and ghostty. Under tmux and screen they are disabled by default, because those multiplexers print passthrough sequences as plain text unless passthrough is enabled. Set `BLUR_KITTY=1` to force images on if your multiplexer is configured for it. Everywhere else, images render as text placeholders.

## Undo

Every edit records an exact inverse, including indentation, line splits, merges, and bracket splits. History is per tab.

## Structure

```text
src/
  main.rs          event loop and renderer
  controls.rs      cursor movement
  normal_mode.rs   normal mode commands
  modes.rs         insert mode, prompts, paste, indentation
  select_modes.rs  visual modes
  helpers.rs       buffer state, highlighting, undo records
  preview.rs       markdown parsing and image protocol
```

## Limitations

- Visual mode is single-line
- No search or replace
- Tables render as raw text

## License

Dual-licensed under MIT or Apache-2.0.

## Credits

- [programmersd21](https://github.com/programmersd21)
- [castlesp5](https://github.com/castlesp5), original author
- [artemtsitronov](https://github.com/artemtsitronov)
