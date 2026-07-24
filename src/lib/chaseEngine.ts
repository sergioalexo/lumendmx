import type { DmxStep, LightAsset } from "./types";

const FRAME_MS = 33; // ~30fps interpolation; well under the 44Hz hardware output rate

/**
 * Steps a single Scene/Chase/FX asset over time, emitting interpolated channel
 * patches. Only one runner should own a given channel at a time -- concurrently
 * triggering two chases that both touch channel 5 will fight over it, which is an
 * acceptable MVP limitation (see PRD review: no channel-ownership/priority model yet).
 */
export class AssetRunner {
  private stopped = false;
  private frameHandle: ReturnType<typeof setTimeout> | null = null;

  constructor(
    private asset: LightAsset,
    private getChannel: (channel: number) => number,
    private onPatch: (patch: Record<number, number>) => void,
  ) {}

  start() {
    this.stopped = false;
    void this.runLoop();
  }

  stop() {
    this.stopped = true;
    if (this.frameHandle) clearTimeout(this.frameHandle);
  }

  private async runLoop() {
    do {
      for (const step of this.asset.steps) {
        if (this.stopped) return;
        await this.runStep(step);
      }
    } while (this.asset.loop && !this.stopped);
  }

  private runStep(step: DmxStep): Promise<void> {
    return new Promise((resolve) => {
      const targets = Object.entries(step.channels).map(([ch, value]) => ({
        channel: Number(ch),
        from: this.getChannel(Number(ch)),
        to: value,
      }));
      const fadeFrames = Math.max(1, Math.round(step.fade_time_ms / FRAME_MS));
      let frame = 0;

      const tick = () => {
        if (this.stopped) return resolve();
        frame += 1;
        const t = Math.min(1, frame / fadeFrames);
        const patch: Record<number, number> = {};
        for (const target of targets) {
          patch[target.channel] = Math.round(target.from + (target.to - target.from) * t);
        }
        this.onPatch(patch);

        if (t < 1) {
          this.frameHandle = setTimeout(tick, FRAME_MS);
        } else {
          this.frameHandle = setTimeout(() => resolve(), Math.max(0, step.hold_time_ms));
        }
      };

      tick();
    });
  }
}
