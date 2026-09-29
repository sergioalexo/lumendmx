# Fixture library

One JSON file per fixture. Every file here is bundled into the app automatically
(no registration step needed) and shows up in the Fixture Patch panel's picker.

## Contributing a fixture

1. Copy `generic-6color-par.json` as a starting point.
2. Filename = the fixture's `id` (kebab-case), e.g. `chauvet-slimpar-q6.json`.
3. Fill in the fields below from the fixture's manual.
4. Load the app -- the Fixture Patch panel validates the file and will print
   errors to the browser/devtools console if something's missing.

You can also import a fixture JSON file at runtime without a PR, from the
"Import fixture JSON" button in the Fixture Patch panel -- useful for testing a
definition, or for a one-off fixture you don't want to upstream.

## Schema

```jsonc
{
  "id": "manufacturer-model-slug",   // unique, matches the filename
  "manufacturer": "Manufacturer",
  "model": "Model Name",
  "type": "LED Par",                 // freeform: "LED Par", "Moving Head", "Strobe", etc.
  "notes": "optional freeform notes",
  "modes": [
    {
      "name": "10-Channel Mode",
      "channelCount": 10,
      "channels": [
        { "offset": 1, "type": "dimmer", "label": "Dimmer" },
        { "offset": 2, "type": "red" }
        // ...
      ]
    }
  ]
}
```

A fixture can list multiple `modes` (e.g. a 6-channel mode and a 10-channel mode)
-- the person patching it in the app picks one when they add it to their rig.

### Channel `type`

One of: `dimmer`, `red`, `green`, `blue`, `white`, `amber`, `uv`, `cyan`,
`magenta`, `yellow`, `strobe`, `function`, `speed`, `pan`, `tilt`, `colorwheel`,
`gobo`, `macro`, `generic`. `label` overrides the display name if you want
something more specific than the type (e.g. `"UV"` instead of `"uv"`).

### `function`-type channels (auto-program / mode-select channels)

Cheap fixtures very often have a channel where low values mean "listen to my
other DMX channels" and higher values switch into built-in auto-chases,
sound-reactive modes, etc. If you don't declare this, a patched fixture can
silently ignore the DMX you're sending it. Use `manualRange` and `defaultValue`
to describe and default it:

```jsonc
{
  "offset": 9,
  "type": "function",
  "manualRange": [0, 50],   // the range that means "plain DMX control"
  "defaultValue": 0,        // written automatically when the fixture is patched
  "notes": "051-100 = built-in color presets, 101+ = auto chases, etc."
}
```

The app writes `defaultValue` for every such channel the moment a fixture is
added to the patch, so it starts out obeying the DMX you send it instead of
running its own program.
