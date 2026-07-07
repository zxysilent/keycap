# Architecture (winit edition)

## Overview

Keycap uses **[winit](https://github.com/rust-windowing/winit)** for cross-platform native windows (0.30), **[tiny-skia](https://github.com/RazrFalcon/tiny-skia)** for CPU-side 2D rendering (0.11), and **[softbuffer](https://github.com/rust-windowing/softbuffer)** for zero-copy framebuffer presentation.

```
┌──────────────────────────────────────────────┐
│  winit Window (.with_transparent(true))      │  ← native OS-level transparency
├──────────────────────────────────────────────┤
│  softbuffer Surface                          │  ← zero-copy pixel buffer
├──────────────────────────────────────────────┤
│  tiny-skia Pixmap (pill rendering)           │  ← round rects, colors
├──────────────────────────────────────────────┤
│  DisplayMode (Timeline / Combo)              │  ← Key → Label + opacity
├──────────────────────────────────────────────┤
│  Capture Thread (rdev::listen)               │  ← Global keyboard hook
└──────────────────────────────────────────────┘
```

## Why winit instead of GPUI?

GPUI 0.2's transparent window support (Direct Composition / `DXGI_ALPHA_MODE_PREMULTIPLIED`) was unreliable on Windows CI runners, producing white backgrounds and requiring 500+ lines of Win32 hacks (DWM, WS_EX_LAYERED, LWA_COLORKEY).

winit's `.with_transparent(true)` has been battle-tested across all three platforms for a decade:

| Platform | winit transparent mechanism |
|----------|---------------------------|
| Windows | WS_EX_LAYERED + per-pixel alpha |
| macOS | NSWindow opaque=NO + transparent bg |
| Linux (X11) | 32-bit ARGB visual |
| Linux (Wayland) | alpha modifier protocol |

**No Win32 hacks needed.**

## Binary Size

| Version | Size |
|---------|------|
| GPUI 0.2 | 13MB |
| winit + tiny-skia + softbuffer | **3.5MB** |

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| winit | 0.30 | Cross-platform window creation |
| tiny-skia | 0.11 | 2D vector graphics (pills) |
| softbuffer | 0.4 | Zero-copy framebuffer |
| rdev | 0.5 | Global keyboard hook |
| tray-icon | 0.14 | System tray (non-Linux) |
| ksni | 0.1 | System tray (Linux) |
| serde + toml | 1 / 0.8 | Config |
| crossbeam | 0.8 | Channel |
| log | 0.4 | Logging |
