# Decisions

One line (or a short paragraph) per call made without stopping to ask, per
`BUILD_PLAN.md`'s ground rules. Newest at the bottom.

## Phase 1 — Foundations

- **Show-file schema scope:** `ShowFile` (schema.rs) only models fields backed
  by real functionality today (universes, patch, workspaces, settings, legacy
  assets). Groups/presets/cuelists/effects/pixel-maps/timelines/MIDI-OSC-mappings
  are added to the schema, with their own migration step, by the phase that
  actually implements them (5, 5, 6, 7, 7, 8, 9). Designing their on-disk shape
  now, before any of the logic that needs it exists, would just mean guessing
  wrong and redesigning it later — the migration system exists precisely so
  this can happen incrementally.
- **Rust core crate layout is populated progressively, not stubbed up front:**
  `showfile/` exists now with real content. `output/` (refactoring `dmx.rs`
  into an `OutputDriver` trait) is Phase 2's task; `engine/` (state/merge/
  playback tick) is Phase 3's; `io/` (MIDI/OSC/timecode moved off the JS side)
  is Phase 9's. Creating empty module directories now would just be decoration.
- **File dialogs via `rfd`, not a Tauri plugin:** New/Open/Save/Save As run
  entirely in Rust commands using the `rfd` crate, so nothing needed adding to
  `src-tauri/capabilities/default.json`.
- **Autosave is driven by a frontend timer, not a Rust background task:** show
  state (patch, legacy assets) currently lives in the frontend Zustand stores,
  not in Rust — that only changes once Phase 3's engine owns programming
  state. So the 60s autosave loop lives in `useShowStore`, which assembles a
  snapshot and hands it to the `showfile_autosave` command to write to disk.
- **`schemaVersion` and all show-file JSON fields are camelCase** (matching
  BUILD_PLAN.md's own wording and the rest of the frontend's JSON), via
  `#[serde(rename_all = "camelCase")]` on every schema struct.
- **Vitest added now, not deferred to Phase 14:** `npm test` is one of every
  phase's exit gates starting now, and there was no test runner at all yet.
  Phase 14 will still add Playwright smoke tests and broaden coverage.
- **`ts-rs` pinned at `9.x`** (latest major is 12) — this is just whatever
  `cargo add` resolved against the `"9"` requirement in Cargo.toml; revisit if
  a later phase's type needs a newer ts-rs feature.
- **ts-rs `export_to` points at a directory, not a single file:** pointing
  every `#[ts(export)]` struct at the same file caused the per-type export
  tests (which run in parallel) to overwrite each other, so only the
  last-finishing type's definition survived. Each type now gets its own file
  under `src/lib/showfile/generated/`, with a hand-written `index.ts` barrel.
- **Left one `npm audit` finding unresolved:** `@vitest/mocker`'s path-traversal
  advisory only has a fix via the Vitest 5 major bump, which is out of scope
  for a docs/save-file phase. It's a dev-only test-mocking codepath with no
  untrusted input anywhere in this repo's test suite.
- **Undo/redo wired into `usePatchStore`, since that's the only "programming
  edit" that exists before Phase 3's real programmer.** `src/lib/undo.ts`'s
  `UndoStack` (command pattern, 100 levels) is written to be reused by the
  Phase 3 programmer for selection/attribute/cue edits, not replaced.
- **Default workspaces are name-only stubs** (Programming/Playback/Live
  AI/Patch-Setup tabs with no window layout yet) — the dockable window grid is
  built up as the window types it can hold exist, not invented speculatively now.

## Phase 2 — Multi-universe output engine

- **Enttec Pro Mk2's second universe is explicitly rejected, not guessed at.**
  There's no physical Pro widget in this project to verify against, and the
  Mk2's dual-universe port-select framing isn't part of the base documented
  protocol I could implement with confidence. `EnttecProDriver::write_frame`
  returns a clear error for `universe_index != 0` rather than silently
  emitting unverified bytes at real hardware. See HARDWARE_CHECKS.md.
- **DMX input receives and decodes but doesn't merge into output yet.**
  BUILD_PLAN Phase 2 asks for input "with a merge option," but Phase 3 owns
  building the real HTP/LTP merge engine. Wiring a second, ad-hoc merge into
  `runner::UniverseOutput` — the same code path the safety-critical FTDI
  driver's frame loop runs through — this late in an already-large phase was
  judged too risky for the payoff; the merge step is Phase 3's to build once,
  properly. See docs/OUTPUT.md.
- **sACN input listeners bind with `SO_REUSEADDR` via the `socket2` crate**
  (std's `UdpSocket` can't set this before bind), so more than one universe's
  multicast listener can share port 5568, which any real multi-universe sACN
  receiver needs.
- **Enum JSON field casing:** serde's `rename_all` on an enum only renames
  *variant* names, not the fields inside struct-like variants — each variant
  of `DriverConfig`/`SacnDestination` needed its own
  `#[serde(rename_all = "camelCase")]` to keep field names consistent with the
  rest of the schema. Caught by a real test failure ("missing field
  `rate_hz`"), not by inspection.
- **Show-file schema v2:** `UniverseConfig.outputPort: Option<String>` became
  `driver: DriverConfig` (tagged enum covering Null/FTDI/EnttecPro/ArtNet/
  sACN), with a migration step and tests for both the "had an FTDI port" and
  "had no port" cases.
