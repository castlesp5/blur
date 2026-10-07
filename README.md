# blur

A modal terminal text editor in Rust. Vim-style keys, syntax highlighting, live markdown preview, and a transparent themed interface.

Built on [ratatui](https://github.com/ratatui-org/ratatui), [crossterm](https://github.com/crossterm-rs/crossterm), [syntect](https://github.com/trishume/syntect), and [opaline](https://crates.io/crates/opaline).

## Features

- Modal editing: normal, insert, visual, and visual-line modes
- Multiple tabs, each with its own undo history and viewport
- Automatic language detection across file names, extensions, shebangs and content
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
./blur              # start screen
./blur path/to/file # open a file directly
```

## Start screen

Launched with no arguments, blur opens on a start screen: the logo, `open file`, `new buffer`, `quit`, and the ten files you opened last.

| Keys | Action |
| ---- | ------ |
| `j` / `k`, arrows | Move |
| `Enter` | Open the highlighted entry, or confirm the path |
| `o` | Focus the path prompt |
| any character | Start typing a path |
| `n` | New buffer |
| `q` / `Esc` | Quit |
| Click | Activate an entry |

The cursor stays hidden on the start screen and only appears while a path is being typed.

Recent files live in `$XDG_STATE_HOME/blur/recent`, or `$BLUR_STATE_DIR/recent` when that is set.

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

### Scrolling

| Keys | Action |
| ---- | ------ |
| Wheel / trackpad | Scroll, the cursor rides along |
| `Shift` + wheel | Scroll sideways |
| `PageDown` / `PageUp` | Half page |
| `Ctrl+D` / `Ctrl+U` | Half page |
| `Ctrl+F` / `Ctrl+B` | Full page |
| `Home` / `End` | Start / end of line |

Scrolling moves the window and the cursor together, so the view never snaps back to the cursor.

### Other

| Keys | Action |
| ---- | ------ |
| `j` / `k`, arrows | Theme picker navigation |
| `Enter` | Apply theme |
| `Esc` | Restore previous theme |

## Mouse

- Click to place the cursor, drag to select
- Click a tab to switch, click its `×` or middle-click it to close
- Click a theme row to apply
- Click the left half of the confirm dialog to quit
- Wheel or trackpad scroll, with the cursor riding along

## Configuration

`$XDG_CONFIG_HOME/blur/config.toml`:

```toml
theme = "catppuccin-mocha"
images = true
```

`images` is optional. Left unset, blur decides from the terminal.

Written when a theme is applied, read at startup. Unknown keys are ignored.

Terminal images are off unless enabled here or with `BLUR_KITTY=1`, because guessing terminal support wrong prints raw escape text on screen.

## Markdown preview

Press `P` with a markdown file open. Headings, lists, quotes, code, and links render live as you type.

Images display inline through the kitty graphics protocol on kitty, WezTerm, and ghostty. Detection reads environment signals rather than querying the terminal, because the query protocol leaves a background reader on stdin when a terminal does not answer, which swallows your keystrokes.

Every other terminal gets a clean `[alt] path` placeholder instead of raw escape codes. Set `images = true` in the config to force images on, which is what you want inside tmux or screen with `allow-passthrough` configured. `images = false` turns them off.

Image support is prepared behind a short deadline, so a slow or stalled terminal multiplexer can never delay or block startup.

## Syntax highlighting

Highlighting uses `syntect` with the `catppuccin-mocha` theme. Lines are fed to the parser with their terminator, so comment and string scopes close correctly at the end of a line. Detection runs in layers, so it works for named files, unsaved buffers, and scripts with no extension:

1. Whole file names: `Makefile`, `Dockerfile`, `Rakefile`, `Cargo.toml`, `.bashrc`, `CMakeLists.txt`, and similar.
2. Path and extension, including compound suffixes such as `.blade.php` and `.d.ts`.
3. Shebangs, with flags handled, so `#!/usr/bin/env -S deno run` resolves correctly.
4. First-line signatures such as `<?php` and `<?xml`.
5. Weighted content scoring over the buffer, for untitled files.

Languages with no bundled grammar borrow the closest one and keep their own name in the status bar: TypeScript and JSX highlight as JavaScript, Kotlin and Dart as Java, Elixir as Erlang, Julia as MATLAB, SCSS and Less as CSS, TOML and INI as YAML-style config, Terraform as YAML, Protobuf as C++, PowerShell as shell, Vue and Svelte as HTML, Zig as C, Crystal as Ruby, Nim as Python, Swift as C++. Anything genuinely unrelated stays plain text rather than being colored wrongly.

Ambiguous extensions are resolved from the file body: a `.h` with `class` or `namespace` is C++, one with `@interface` is Objective-C, and otherwise it is C. A `.m` containing `function` is MATLAB, otherwise Objective-C.

Drop any `.sublime-syntax` or `.tmLanguage` file into `$XDG_CONFIG_HOME/blur/syntaxes` and it is loaded at startup for exact grammars.

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
