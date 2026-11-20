# Windows acceptance checklist

Run the optimized executable on Windows 10/11 x64. These are manual acceptance
steps, **not claims that every hardware/Explorer case has been automated**.
Use copies of documents; MdViewer must never alter their contents or timestamps.

## Packaging, runtime and window

- [ ] Copy only the exe to a fresh folder and run without source/assets/Node.
- [ ] On a clean machine with WebView2, launch successfully without a companion
  loader DLL or VC redistributable installer.
- [ ] In a disposable VM without WebView2, get a clear native message with the
  Microsoft download URL; No exits, Yes opens the browser, no automatic install.
- [ ] File Properties shows product/file version and icon; taskbar/titlebar icon
  is correct. Help > About shows app and actual WebView2 versions.
- [ ] Resize and maximize: native menu, tabs, optional find, tiny status bar;
  content fills all remaining width/height with roughly 14 px side gutters.
- [ ] With different tabs active, title bar reads `MdViewer | Path: <absolute
  full file path>` and changes when switching/closing tabs.
- [ ] Large tables/code have their own horizontal overflow; content scrolls
  vertically without a centered/max-width column.
- [ ] Move between 100%, 150% and 200% DPI monitors. Close/relaunch: normal bounds
  and maximized state restore. Disconnect a monitor: window remains reachable.
- [ ] Launch while offline; verify no content requires a network connection.

## File opening and tabs

- [ ] Open File button, File > Open File, File > New Tab, plus, Ctrl+O and Ctrl+T
  all open a native multi-select dialog. Cancel leaves tabs unchanged.
- [ ] Test `.md`, `.markdown`, `.mdown`, `.txt`, upper-case extensions and All
  files. Text and explicitly chosen other extensions render as Markdown.
- [ ] Select several files; order is stable. Reopen through canonical/relative/
  different-case/hard-link paths: activate the existing tab, not a duplicate.
- [ ] Long names ellipsize; hover shows the complete path; many tabs show working
  overflow arrows; selected tab remains visible.
- [ ] Click tabs, tab close buttons, and middle-click. Ctrl+W closes the selected
  tab. Ctrl+Tab/Ctrl+Shift+Tab cycle and wrap even when content has focus.
- [ ] Drag tabs both directions, including after scrolling overflow. Reopen:
  reordered tab order persists.
- [ ] Right-click each context action: Close, Close Others, Close Tabs to the
  Right, Copy Full Path (paste elsewhere to verify), Open Containing Folder.
- [ ] Close active middle/last/only tab: right neighbor, then left, then Open File
  empty state. Close All Tabs leaves no blank editable document.
- [ ] Drop multiple supported files onto the window and onto the exe icon.
  Unsupported drop extensions are ignored without interrupting work.
- [ ] Launch with several arguments containing spaces/Unicode. Launch with a
  relative path from a different working directory; resolve it correctly.
- [ ] With an instance running, repeat launches (including simultaneous startup,
  duplicate files and no arguments). One window receives the paths. Minimized
  window restores; focus or taskbar attention follows Windows foreground rules.

## Markdown, local resources and read-only behavior

- [ ] Open `tests\fixtures\features.md`: tables, task lists, strike, footnotes and
  backlinks, sub/sup, ins/mark, definitions, abbreviations, emoji, anchors, TOC.
- [ ] Duplicate headings get distinct anchors, including collisions such as
  `Foo`, `Foo`, `Foo-1`.
- [ ] Front matter is hidden; normal horizontal rules remain visible.
- [ ] Task checkboxes cannot be toggled by mouse, keyboard or labels.
- [ ] Inline HTML presentation/details work; scripts, forms, edit controls,
  contenteditable, event attributes, authored styles and embeds do not.
- [ ] KaTeX inline/block math and required fonts work fully offline. Invalid
  expressions show safe source/diagnostics.
- [ ] Mermaid flowchart, sequence, class, state and other bundled diagram types
  work offline; malformed/over-limit diagrams show a safe diagnostic. Document
  directives, clickable callbacks and links cannot run commands.
- [ ] Named code languages highlight; unknown/plain code remains readable.
  Visit a plain-only document first: optional renderers have not loaded.
- [ ] Relative Markdown links open/deduplicate tabs, including `..` siblings and
  fragments. A new relative target opens as a new tab; an already-open target
  activates its existing tab. Same-document anchors/TOC/footnotes scroll correctly.
- [ ] Right-click a link: Open Link follows it, Copy Link Address puts its
  resolved URL on the clipboard, and Copy Link Text copies the rendered label.
- [ ] Explicit HTTP/HTTPS clicks open the browser, not an embedded web page.
  JavaScript, file, data and other schemes are blocked.
- [ ] Hover and keyboard-focus document links: the bottom status bar shows the
  link target URL and restores the active document path on leaving the link.
- [ ] Relative raster images within the document folder resolve, including spaces
  and Unicode. Remote images, document SVG, traversal and junction escapes do
  not load. UNC and mapped-network documents are rejected explicitly.
- [ ] Select/copy text normally. No editor, Markdown save/save-as, split view,
  recent-files list, plugin manager or auto-update is available. Ctrl+S and
  browser context menus/devtools do not expose those features.
- [ ] In Dark or Sepia with zoom above 100%, select File > Print or Ctrl+P.
  Toggle Add page numbers, then choose a printer. Verify only the document
  prints on white, with a centered `Page X of Y` footer only when selected. Cancel the
  printer dialog and verify the viewer can print again.
- [ ] Use File > Save as PDF or Ctrl+Shift+S on a multi-page document containing
  a local image, a table, math and a Mermaid diagram. Verify Portrait is selected
  each time; export both orientations and verify A4, 15 mm margins, white
  pages, correct numbering on every page when enabled, and no viewer chrome.
  Cancel the file picker, try an invalid destination and verify failures are
  reported. Verify the input file, current tab and theme do not change.

## Theme, zoom and find

- [ ] View > Theme Light, Dark and Sepia update text, surfaces, links,
  blockquotes, stripes, code, selection, highlights and visible Mermaid diagrams.
- [ ] Custom displays 14 labeled color pickers and hex boxes: page background,
  text, surface, accent, links, borders, code background, table stripe, selection
  background/text, muted text and syntax keyword/string-number/comment colors.
- [ ] Live preview updates the palette and diagrams. Invalid hex is rejected;
  contrast below WCAG AA 4.5:1 produces a warning. Apply persists; Close/Esc
  restores the previous theme and colors. Reset to Sepia fills every color field
  and updates the preview without closing the dialog; Apply commits Sepia and
  Close discards it. The theme-menu reset selects Sepia directly.
- [ ] Zoom menu, Ctrl+=, Ctrl++, Ctrl+-, Ctrl+0 and Ctrl+wheel agree, step by 10%,
  clamp at 50/300 and persist. UI remains compact; gutters scale with content.
- [ ] Ctrl+F reveals a slim bar. Typing updates all matches/count; current match
  differs; matches spanning inline emphasis are found.
- [ ] Enter/Shift+Enter and F3/Shift+F3 navigate/wrap. Case toggle, empty query,
  no matches, Unicode and Esc work. Search covers only active rendered content,
  not tabs, hidden front matter or other files.
- [ ] Search remains correct after tab switch, reload, zoom, theme changes and
  diagrams/fonts finishing layout.

## Persistence, reload and failures

- [ ] Reopen multiple tabs: order, selected surviving tab, scrolls, theme,
  custom colors, zoom and window state restore. Only the active tab renders.
- [ ] Delete some saved files before launch: no tab/message/banner/dialog;
  the next saved session no longer lists them.
- [ ] Delete the saved active file; nearest surviving tab becomes selected.
- [ ] Direct-write and temp-file+rename saves reload after about 200 ms and
  preserve scroll. Rapid saves do not display a stale intermediate revision.
- [ ] Change an inactive file: no inactive render; activating shows latest bytes.
- [ ] Delete/move a runtime-open file: absolutely no deletion indicator; last
  render remains. Recreate it: reload automatically. Close clears retained data.
- [ ] Move/delete the entire document folder and recreate it; recovery resumes
  without recursive watching or idle polling. Unrelated sibling saves do nothing.
- [ ] Test UTF-8, UTF-8 BOM, UTF-16 BOM, Windows-1252 and malformed encoding.
  Fallback/loss is reported, not a crash.
- [ ] Test sharing violations, permissions failures, missing CLI files, corrupt
  JSON, unwritable settings, bad image references and watcher errors. Failures
  are surfaced/logged; exit-save failure offers an explicit exit-without-saving
  decision. Invalid settings are backed up.
- [ ] Confirm documents were never written. Settings update atomically after
  changes and on exit. Logs rotate; no telemetry or external error reporting.

## Association and performance

- [ ] Set as Default registers per-user ProgID/open command/DefaultIcon/
  Applications/SupportedTypes/OpenWithProgids for all three extensions, without
  an admin prompt. Explorer is notified; instructions and Default Apps open.
- [ ] Choose the default manually. Explorer Open With/double-click passes a
  quoted filename correctly. Registration alone does not force UserChoice.
- [ ] Remove Association reports success/failure, removes only MdViewer-owned
  entries, preserves other apps and registrations at a different exe location.
  Choose another default in Windows settings afterward.
- [ ] Open several-MB prose, long lines, dense tables/lists and code/math-heavy
  documents. UI/find/tab input remains responsive; no arbitrary source truncation.
- [ ] Switch/close/reload many times: inactive DOM stays cheap; memory stabilizes
  subject to bounded HTML caches and pinned absent-file renders.
- [ ] Run `tools\measure.ps1` for plain and feature fixtures. Record exe bytes,
  startup samples, native and WebView2 process-tree working set/private bytes,
  and idle CPU over an interval. Distinguish cold from warm runs.
- [ ] Compare actual results with 6 MB/300 ms goals; report misses honestly.
  No idle animation loop or file polling should keep CPU busy.
