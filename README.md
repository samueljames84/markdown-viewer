# MdViewer

A lightweight, **view-only** Markdown viewer for Windows 10/11 x64. Native menus,
Notepad++-style tabs, full-width content, and offline browser-quality rendering.
There is no document editor, Markdown save command, extension system,
telemetry, updater, or installer.

[Release notes](RELEASE_NOTES.md)

## Build and run

Prerequisites:

- Stable Rust with the `x86_64-pc-windows-msvc` target.
- Visual Studio 2022 Build Tools: **Desktop development with C++**, including the
  MSVC x64 tools and Windows SDK/resource compiler.
- The installed [Microsoft WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/).
  It is not bundled. Missing-runtime startup displays a native explanation and
  an optional link to Microsoft's download page.

```powershell
cargo build --release
.\target\x86_64-pc-windows-msvc\release\mdviewer.exe
.\target\x86_64-pc-windows-msvc\release\mdviewer.exe "D:\Notes\one.md" "D:\Notes\two.md"
```

If Rust is installed but `cargo` is not on the current PowerShell PATH, first run
`$env:PATH="$env:USERPROFILE\.cargo\bin;$env:PATH"`.

Distribute **just `mdviewer.exe`**. Scripts, fonts, styles, icons, licenses and
renderers are compressed and embedded at build time. The WebView2 loader and
MSVC CRT are statically linked. Cargo's release profile uses size optimization,
LTO, one codegen unit, abort-on-panic and stripping.

Node/npm is **not** needed to build from the checked-in assets or to run the app.
Cargo needs its crates available locally, or internet access to obtain them at
build time.

## Using the viewer

- **Ctrl+O / Ctrl+T / +**: multi-select files. The dialog offers `.md`,
  `.markdown`, `.mdown`, `.txt`, and All files.
- **Ctrl+Tab / Ctrl+Shift+Tab**: cycle; **Ctrl+W**, middle-click or **x**: close.
  Drag tabs to reorder. Right-click for close others/right, copy path, and folder.
  Existing files, including hard-link aliases, are deduplicated.
- **Ctrl+= / Ctrl++ / Ctrl+- / Ctrl+0**, or **Ctrl+wheel**: content zoom,
  50–300% in 10% steps. The small status bar shows zoom and the active file
  path, or a link target while hovering/focusing a document link.
- **Ctrl+F**: incremental literal find, case toggle, all-match/current-match
  highlights and count. **F3 / Shift+F3**, or Enter/Shift+Enter in the find input:
  next/previous. **Esc** closes find.
- **F5**: reload. Changes made in other applications reload automatically.
- **View > Theme**: Light, Dark, Sepia or Custom. Custom provides 14 color
  controls with color pickers and hex entry: page background, text, surface,
  accent, links, borders, code background, table stripe, selection background
  and text, muted text, and syntax keyword/string-number/comment colors. It has
  live preview, Apply/Close, contrast warnings below the WCAG AA 4.5:1
  recommendation, and Reset to Sepia. Reset fills every color with the Sepia
  palette and keeps the dialog open; Apply commits it, while Close discards
  changes. Sepia is also available directly from the theme menu.
- Drop supported files onto the window or executable. Additional launches forward
  their arguments to the first instance. Windows foreground restrictions may
  cause taskbar attention instead of stealing focus.
- Relative Markdown/text links open tabs; fragments scroll within documents.
  Right-click a link for Open Link, Copy Link Address or Copy Link Text.
  Explicit HTTP/HTTPS clicks and Open Link open your browser. Local relative
  images are restricted to the document's own folder and descendants.
- The native title bar shows `MdViewer | Path: <absolute file path>` for the active
  tab. Hovering or keyboard-focusing a document link shows its target URL in the
  bottom status bar; leaving the link restores the current file path.

### Print and save as PDF

- **File > Print** (`Ctrl+P`) opens print options, then the Windows printer
  dialog. Choose the printer and its settings there.
- **File > Save as PDF** (`Ctrl+Shift+S`) exports directly without a PDF printer.
  Choose **Portrait** or **Landscape**, then select **Choose PDF File...**.
  Each export starts in Portrait. Both orientations use A4 with 15 mm margins.

Both commands output only the active document on a white background, regardless
of viewer theme or zoom. **Add page numbers** is off by default; enable it for
a centered `Page 1 of 10` footer. Neither command changes the original Markdown.

### Rendering

markdown-it supports tables, read-only task checkboxes, strikethrough, footnotes,
sub/superscript, insertion, marking, definitions, abbreviations, emoji, unique
heading anchors and `[toc]`. Leading YAML front matter is hidden, never executed.
Inline HTML is sanitized. KaTeX renders `$...$`/`$$...$$`; Mermaid renders fenced
`mermaid` blocks; highlight.js handles named fenced code languages.

Optional renderers load only when needed. One rendering worker parses Markdown,
math and highlighting off the UI thread; sanitized HTML is inserted in batches.
Only the selected document occupies the content DOM. Inactive tabs retain cheap
metadata and an approximately 32 MiB LRU HTML cache; absent files' last-good
renders are pinned until they reappear or close.

UTF-8 (including BOM) and BOM-marked UTF-16 are supported. Other byte sequences
fall back to Windows-1252; fallback/lossy decoding is reported in the small status
area and log. There is deliberately no source-editing surface.

### Offline and read-only boundaries

Strict CSP prohibits remote scripts, inline scripts, eval, frames and forms.
WebView2 requests are restricted to the embedded synthetic HTTPS origin;
permissions, downloads, browser menus, devtools and built-in browser shortcuts
are disabled. Printing and PDF export use explicit MdViewer commands.
Original HTML cannot supply styles or editable controls.
Narrowly sanitized generated KaTeX/Mermaid markup may use inline styles.

Remote images, file/JavaScript/data links, document SVG, UNC/network-drive files,
path traversal, alternate streams, and image symlink/junction escapes are
blocked. External links, Explorer folders, and Windows Default Apps settings
open only through user actions. The system Evergreen runtime's servicing remains
Windows-managed; MdViewer does not implement an updater.

Malformed math/diagrams show source with a render diagnostic, not executable
content. Mermaid is strict and non-interactive, with document configuration
directives disabled. Its safety limits are 200,000 source characters and 2,000
edges per diagram; these do not limit Markdown file size.

## Sessions and file watching

Settings live at `%APPDATA%\MdViewer\settings.json`: tab order, active index,
per-tab pixel scroll offset, theme/colors, zoom, normal window bounds, DPI scale,
and maximized state. Paths are lossless UTF-16 code-unit arrays so even unusual
Windows filenames round-trip. Changes save with a 400 ms debounce and atomic
replacement; exit flushes pending state.

Saved paths that no longer exist are silently discarded at startup. Runtime
deletion/movement keeps the last render **without any deletion indicator**;
reappearance reloads it. Other failures are logged and surfaced. Corrupt settings
are backed up before defaults are used.

`notify` uses shared **non-recursive parent-folder watches**, immediately filtered
to open paths, with 200 ms debounce. This is intentional: Windows notifications
are directory-based, and literal file-only watches cannot reliably follow
atomic saves or delete/recreate. Missing-folder recovery temporarily watches the
nearest surviving ancestor. There is no idle file polling.

Logs rotate under `%APPDATA%\MdViewer` (current plus two previous files, around
256 KiB each). The necessary WebView2 profile/cache is under
`%LOCALAPPDATA%\MdViewer\WebView2`; nothing is extracted beside the executable.

## File associations

**File > Set as Default for .md** registers MdViewer under HKCU for `.md`,
`.markdown` and `.mdown`, notifies Explorer, explains the remaining step, and
opens Windows Default Apps. Select MdViewer there: Windows 10/11 deliberately
blocks apps from forcing UserChoice.

**Remove Association** removes only registrations owned by this executable
location. It does not rewrite other applications' entries or Windows UserChoice.
If MdViewer was the selected default, choose another app in Windows settings.
Moving/renaming the portable executable requires registering its new location.

## Project and development

```text
Cargo.toml / Cargo.lock / rust-toolchain.toml
.cargo\config.toml         MSVC x64 and static CRT
build.rs                  compressed embedded assets and Windows resources
src\main.rs               GUI entry point and top-level errors
src\app.rs                window, native menus, commands, protocols, persistence
src\documents.rs          document identity, decoding, constrained image access
src\settings.rs           versioned settings and rotating logs
src\watcher.rs            event-driven, filtered watch lifecycle
src\windows.rs            native colors/clipboard, associations, secured pipes
frontend\                 vanilla UI, find, themes and worker source
frontend\vendor\          committed offline bundles, fonts and version/hash manifest
resources\                original icon and PerMonitorV2 manifest
tests\                    rendering fixtures and browser checks
tools\                    development-only bundling, native tests and measurements
```

To regenerate vendor assets, use Node 22+:

```powershell
npm --prefix tools ci
npm --prefix tools run vendor
.\tools\notices.ps1
```

Do not regenerate assets while Cargo is embedding them. Renderer versions are
locked in `tools\package-lock.json`; hashes and exact direct versions are recorded
in `frontend\vendor\manifest.json`. markdown-it 14.3.3 is intentional: it contains
the current fixes while remaining compatible with the selected emoji plugin;
the incompatible 15.x plugin interface is not shimmed.

Validation:

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
npm --prefix tools test
cargo build
npm --prefix tools run test:native
cargo build --release
.\tools\measure.ps1
.\tools\measure.ps1 -Document .\tests\fixtures\features.md
```

Browser tests use installed Microsoft Edge through development-only Playwright.
Native tests use a debug-only loopback CDP port, fresh temporary APPDATA/profile
directories and temporary documents; release builds ignore that debug setting.
Tests never register real default associations. See [MANUAL_TESTS.md](MANUAL_TESTS.md)
for the complete physical Windows/Explorer acceptance checklist.

The [MIT license](LICENSE) covers original code. Dependency and font licenses are
in [THIRD_PARTY_NOTICES.txt](THIRD_PARTY_NOTICES.txt), also embedded in the binary.

## Measurements

The optimized executable is **4,321,280 bytes (4.121 MiB)**, below the 6 MB goal.
Measured on Windows 10 build 19045, Intel i7-11850H, Rust stable 1.99.1 and
WebView2 157.0.4341.46. Each case restores two tabs, renders only the active one,
and uses three launches with fresh WebView2 profiles. Idle sampling follows a
five-second settle and covers ten seconds. Values below are medians:

| Measurement | Plain Markdown | Math, Mermaid and code |
| --- | ---: | ---: |
| Shell ready | 337 ms | 372 ms |
| Active content ready | 380 ms | 829 ms |
| Native host working set | 22.62 MiB | 23.91 MiB |
| Native host private bytes | 5.13 MiB | 4.83 MiB |
| Entire process-tree working set | 347.13 MiB | 403.67 MiB |
| Entire process-tree private bytes | 161.98 MiB | 212.36 MiB |
| Process-tree idle CPU, one core = 100% | 0.625% | 2.655% |

The tree includes seven native/WebView2 processes. Working-set sums can
double-count shared pages; private bytes are reported separately rather than
hiding browser costs. Individual idle CPU samples ranged from 0% to 2.809% of
one logical core: WebView2 housekeeping means zero CPU is not guaranteed, even
though the application has no idle file polling.

**The 300 ms startup goal was not met.** Active-content samples ranged from
368-479 ms for plain Markdown and 828-848 ms with optional renderers. Markers
start at Rust `run()`, not OS process creation, and math readiness waits for its
stylesheet and fonts. Filesystem/runtime caches may be warm; these are not
verified true cold-start results. Clean-machine and cold-boot checks remain
manual acceptance work.

Validation passed: formatting, strict Clippy, five Rust tests, seven browser
tests, real Windows/WebView2 integration and the release build. The generated
3+ MiB document's maximum 20 ms UI-heartbeat gap was 45 ms in the final browser
run (required below 250 ms). Browser tests also cover diagram-label find and
exclude hidden duplicate MathML from matches. VM/runtime-missing, real Default
Apps changes, native color selection and multi-monitor/DPI checks are documented
in the manual checklist, not claimed as automated passes.

## Assumptions and boundaries

1. Windows 10/11 x64 with supported system WebView2; no bundled runtime.
2. One portable distributable, but normal per-user settings/logs/WebView2 cache.
3. Joplin-like syntax/rendering, not Joplin notebooks or exact pixel parity.
4. Reliable atomic-save/reappearance behavior takes priority over literal
   file-only watches; watches are non-recursive and filtered.
5. Local documents/images only. Explicit browser links are allowed; remote
   content is never loaded inside the viewer.
6. All-files selections and `.txt` files render as Markdown. New Tab opens a
   picker, never an editor.
7. Default Light theme/100% zoom; closing selects the right neighbor, then left.
   The last command-line path becomes active.
8. Scroll restoration preserves pixel offsets, not semantic paragraph positions
   after arbitrary content changes. Deleted-file renders survive only in memory,
   not across restarts.
9. Rendering completeness and honest measurements take precedence over
   aspirational size/startup numbers.
