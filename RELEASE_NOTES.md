# MdViewer v0.1.1

## What's new since v0.1.0

- **Print and save as PDF.** Use **File > Print** (`Ctrl+P`) to open the
  Windows printer dialog, or **File > Save as PDF** (`Ctrl+Shift+S`) to export
  directly without a PDF printer. PDF export uses A4 with 15 mm margins and
  offers Portrait (the default each time) and Landscape. Both commands output
  only the active document on a white background and leave the Markdown
  unchanged. Enable **Add page numbers** for a centered `Page 1 of 10` footer.
- **Sepia and custom themes.** Edit 14 colors with live preview. **Reset to
  Sepia** fills the entire palette without closing the theme dialog, so you
  can review the colors before applying them.
- **Clearer paths and better link controls.** The title bar shows the active
  file's absolute path. Hover over or focus a link to see its destination in
  the status bar. Right-click for **Open Link**, **Copy Link Address**, or
  **Copy Link Text**. Relative Markdown links open in a new tab or switch to
  the existing tab.

## Requirements

Windows 10 or 11 (x64) with the [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/).
Download and run `mdviewer.exe`; no installer is needed.
