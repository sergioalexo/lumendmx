# Fixture schema, importers, and patch (Phase 4)

See `src/fixtures/library/README.md` for the schema itself (fields, channel
types, the optional 16-bit/capabilities/multi-cell/physical additions). This
doc covers the importers and the patch model that sit around it.

## Importers

- `src/lib/fixtures/importers/ofl.ts` — [Open Fixture Library](https://open-fixture-library.org/)
  JSON. Built against OFL's documented fixture format and a real fixture
  (Cameo Auro Spot 300) fetched from the OFL GitHub repo. Handles fine
  (16-bit) channels via `fineChannelAliases`, wheel slot colors/images via the
  `wheels` object, and the full capability-type vocabulary (Pan/Tilt,
  ColorIntensity, ShutterStrobe, WheelSlot, Prism, Focus/Zoom/Iris/Frost,
  Speed, Effect, Maintenance, NoFunction). Matrix/pixel fixtures
  (`matrix`/`templateChannels`) are rejected with a clear error — that needs
  Phase 7's Pixel Map, not this importer.
- `src/lib/fixtures/importers/qlcplus.ts` — QLC+ `.qxf` (XML, via
  `DOMParser`). QLC+'s `Group` is a broad category (Colour/Beam/Gobo/Pan/
  Tilt/Shutter/Speed/Effect/Maintenance/Nothing) with no structured color
  name or wheel-slot data, so a specific color (red/green/blue/...) is
  inferred from the channel's own name when the group is generically
  "Colour" — an honest limitation of the source format. A `Capability`'s
  `Preset` attribute (e.g. `ShutterOpen`, `PanMSB`) refines the type further
  when present. Two consecutive `<Channel>` entries in a `<Mode>` sharing a
  `Group` name with `Byte="0"` then `Byte="1"` are recognized as a 16-bit
  coarse/fine pair, by QLC+ convention.

Both are unit-tested against realistic (not toy) fixture data — see their
`.test.ts` files. Neither ships "the full OFL/QLC+ catalog" bundled; see
DECISIONS.md for why.

## Patch model (schema v3)

`PatchedFixture` (`src/store/usePatchStore.ts`, mirrored in
`src-tauri/src/showfile/schema.rs`) now carries:

- `universe`: which universe this fixture is patched into. Every fixture was
  implicitly universe 1 through Phase 3; Setup can now manage more than one.
- `fixtureNumber`: a stable, user-assignable number the command line's
  `1 THRU 8` etc. resolves against (see docs/ENGINE.md) — replaces Phase 3's
  "just patch order" placeholder. `usePatchStore.nextFixtureNumber()` suggests
  the next unused one; the patch form lets you override it.
- `invertPan`/`invertTilt`/`swapPanTilt`: booleans, editable from a patched
  fixture's detail panel when it has pan/tilt channels. Not yet consumed by
  anything (there's no motion-control math to apply them to until fixtures
  are addressed by attribute rather than raw channel), but the data is there
  for when that lands.

The patch panel (`FixturePatchPanel.tsx`) adds, over Phase 1-3's version:

- A universe picker and auto-suggested next-free address (computed from
  what's already patched in that universe), recomputed as you change
  fixture/mode/universe unless you've typed your own address.
- Address collision detection: patching or re-patching into an address range
  that overlaps another fixture in the same universe is blocked with a
  message naming the conflicting fixture(s).
- Re-patch: change an existing fixture's universe/address in place from its
  detail panel, instead of unpatch-then-repatch.
- Live channel values now come from `useUniverseChannelsStore`, which caches
  the engine's per-universe frames — needed once a fixture could be on any
  universe, not just the one `useDmxStore` tracks for the top bar's
  quick-connect UI.

Not built: a drag-onto-a-universe-grid visual patch view. Collision
detection, auto-addressing and re-patch cover the same underlying mistakes a
grid view prevents; the visual is additive polish layered on the same
operations, not a different capability.
