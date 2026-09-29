# The lighting engine

`src-tauri/src/engine/` computes what actually goes out on the wire: it merges
the programmer and every running playback into a final per-universe buffer at
a fixed 44Hz, independent of the UI's frame rate.

## Why channel-based, not fixture/attribute-based

The engine's `Layer` type (`state.rs`) only knows about `(universe, channel)
-> value`. Resolving "fixture 3's red channel" into an actual DMX address is
static configuration data (the fixture library + patch), not timing-critical,
so that resolution stays in the frontend, which already has the fixture
library (`src/lib/fixtures/`) — duplicating it in Rust would just be a second
copy to keep in sync. Only the *resolved* result crosses into Rust:

- `engine_set_htp_channels(universe, channels)` — which channels merge via HTP
  (a fixture's dimmer channel) vs LTP (everything else). Recomputed and pushed
  whenever the patch or fixture library changes (`useShowStore.ts`'s
  `syncEnginePatch`).
- `engine_set_patch_index(entries)` — fixture number -> (universe, intensity
  channel), what the command line's `1 THRU 8 @ 50` resolves against. Fixture
  numbers are just 1-based patch order for now; real fixture ID assignment is
  Phase 4's Patch screen.

## Merge model

`merge::merge_universe` (see its module docs and tests): intensity channels
are HTP (max across every layer, regardless of priority); everything else is
LTP (the highest-priority layer with an entry for that channel wins outright).
The programmer's priority is `i64::MAX`, so "programmer > playbacks" is just a
consequence of the general rule, not a special case.

## Playback layers

Every triggered Scene/Chase/FX (`src/store/useLibraryStore.ts`'s `trigger`,
or the Live AI console) becomes one playback layer
(`legacy_playback::RunningPlayback`), advanced every tick instead of on its
own `setTimeout` — this is what BUILD_PLAN Phase 3 means by "retire
chaseEngine.ts; replace it with a Rust playback that's frame-accurate". Two
playbacks that touch the same channel now go through the same HTP/LTP merge
as everything else, instead of racing to call `applyPatch` last the way the
old runner did.

A non-looping playback (a "Scene") keeps its last values live after it
finishes ticking — matching `LightAsset`'s own "Scenes always apply steps[0]
once" contract (a static look that persists) rather than reverting once its
`RunningPlayback` stops. `stop_asset`/re-triggering the same id is what
actually removes a layer.

## What isn't here yet

- **Cuelists, groups, presets** (Phases 5/6): the command line's
  `GROUP`/`RECORD CUE`/`UPDATE`/`DELETE CUE`/`COPY`/`MOVE` all parse correctly
  (and are unit-tested) but execute as a clear "not implemented yet" error.
  Guessing at semantics those phases haven't designed would just mean
  redoing it.
- **An attribute encoder bar / full manual programmer UI**: `CommandLine.tsx`
  is the only manual programmer surface today. The Onyx-style attribute bar
  (BUILD_PLAN section 2) is progressive UI work built up as later phases add
  the window types it needs.
- **DMX input merged into output**: `output::input` receives and decodes
  Art-Net/sACN but doesn't feed it into this engine's merge — see
  docs/OUTPUT.md's scope note.
