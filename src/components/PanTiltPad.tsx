import { useRef } from "react";

const PAD_SIZE = 160;

/** A 2D pan/tilt control surface (BUILD_PLAN Phase 5): drag to set both
 * channels at once. Pan is horizontal (0 left, 255 right), tilt is vertical
 * (0 top, 255 bottom) -- screen-space, not physical degrees, since the
 * engine only knows raw channel values (see docs/ENGINE.md's channel-based
 * design note); a fixture's own invert/swap patch settings (Phase 4) are
 * applied before values reach here. */
export function PanTiltPad({
  pan,
  tilt,
  onChange,
}: {
  pan: number;
  tilt: number;
  onChange: (pan: number, tilt: number) => void;
}) {
  const padRef = useRef<HTMLDivElement>(null);

  const setFromEvent = (e: { clientX: number; clientY: number }) => {
    const rect = padRef.current?.getBoundingClientRect();
    if (!rect) return;
    const x = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
    const y = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));
    onChange(Math.round(x * 255), Math.round(y * 255));
  };

  return (
    <div className="flex flex-col items-center gap-2">
      <div
        ref={padRef}
        onPointerDown={(e) => {
          e.currentTarget.setPointerCapture(e.pointerId);
          setFromEvent(e);
        }}
        onPointerMove={(e) => e.buttons === 1 && setFromEvent(e)}
        className="relative shrink-0 cursor-crosshair rounded-md border border-border bg-muted/30"
        style={{ width: PAD_SIZE, height: PAD_SIZE }}
      >
        <div className="absolute left-1/2 top-0 h-full w-px bg-border" />
        <div className="absolute left-0 top-1/2 h-px w-full bg-border" />
        <div
          className="absolute h-3 w-3 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-primary bg-background shadow"
          style={{ left: (pan / 255) * PAD_SIZE, top: (tilt / 255) * PAD_SIZE }}
        />
      </div>
      <div className="flex gap-3 text-xs text-muted-foreground">
        <span>Pan {pan}</span>
        <span>Tilt {tilt}</span>
      </div>
    </div>
  );
}
