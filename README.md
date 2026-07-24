# LumenDMX

AI-driven DMX512 lighting controller for live electronic music. Natural-language
scene/chase generation (Programmer Mode) plus real-time AI tweaks to the running
universe during a set (Live AI Mode), with a hardware output loop that never
glitches regardless of what the UI or an LLM is doing.

## Stack

- **Frontend:** React 19 + Vite + TypeScript + Tailwind CSS v4, Zustand for state.
- **Desktop shell:** Tauri v2 (Rust).
- **Hardware:** `serialport` crate driving an FTDI USB-to-RS485 adapter; a dedicated
  Rust thread reads a shared `Arc<Mutex<[u8;512]>>` universe and blasts DMX512
  frames at ~44Hz, fully decoupled from the UI/AI (see `src-tauri/src/dmx.rs`).
- **MIDI:** Web MIDI API in the frontend, with hot-swap support and a "MIDI Learn"
  binding flow (`src/store/useMidiStore.ts`).
- **AI:** pluggable adapters for Ollama (local), OpenAI, Gemini, and Anthropic, all
  targeting one shared JSON schema (`src/lib/ai/`).
- **Design system:** ported from
  [sergioalexo/design-system](https://github.com/sergioalexo/design-system) -- HSL
  token pairs (light/dark) mapped through Tailwind v4 `@theme inline` in
  `src/index.css`, CVA-based primitives in `src/components/ui/`. Dark is the only
  active mode (`<body class="dark">` in `index.html`); the light tokens stay
  defined for parity with the shared system but aren't wired to a toggle.

## Running it

```bash
npm install
npm run tauri dev
```

Requires the Rust toolchain (`rustc`/`cargo`) for the Tauri backend. `npm run dev`
alone runs just the Vite frontend in a regular browser for UI iteration -- native
calls (serial ports, DMX output) no-op gracefully outside the Tauri runtime.

## Known MVP limitations

Carried over from the PRD review, not yet addressed:

- Single DMX universe (512 channels), no Art-Net/sACN network output.
- No fixture/personality library -- channels are raw 1-512 numbers, not
  pan/tilt/gobo semantics.
- AI API keys are stored in `localStorage`, not the OS keychain.
- Concurrently triggering two Scenes/Chases that touch the same channel will fight
  over it -- no channel-ownership/priority model yet.
- OTA update rollback/skip-during-a-show flow is not implemented.
