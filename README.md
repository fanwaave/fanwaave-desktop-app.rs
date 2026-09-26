# fanwaave-desktop-app.rs

Native Rust desktop app. No webviews, no React. UI rendering is isolated in `src/ui.rs`.

## Shared desktop runtime

Drag-and-drop lifecycle and accepted-drop integration are owned by the pinned `ores-dnd-core` dependency. The Fanwaave desktop controller stays a thin native adapter and delegates accepted-drop side effects to the shared ORES forms -> opto-sync -> ores-otel port sequence. Product/UI code must not duplicate that ordering or emit raw drag payload data through telemetry.
