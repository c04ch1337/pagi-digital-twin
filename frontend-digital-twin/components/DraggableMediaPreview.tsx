import * as React from 'react';
import type { ActiveVideoSource } from '../hooks/useMediaStream';

type Props = {
  stream: MediaStream;
  activeVideoSource: ActiveVideoSource;
};

export default function DraggableMediaPreview({ stream, activeVideoSource }: Props) {
  const videoRef = React.useRef<HTMLVideoElement | null>(null);
  const containerRef = React.useRef<HTMLDivElement | null>(null);

  const [pos, setPos] = React.useState<{ x: number; y: number }>(() => ({
    x: Math.max(20, window.innerWidth - 240),
    y: Math.max(20, window.innerHeight - 190),
  }));

  const dragRef = React.useRef<{
    dragging: boolean;
    offsetX: number;
    offsetY: number;
  }>({ dragging: false, offsetX: 0, offsetY: 0 });

  React.useEffect(() => {
    const el = videoRef.current;
    if (!el) return;

    el.srcObject = stream;
    el.muted = true;
    el.playsInline = true;
    const play = async () => {
      try {
        await el.play();
      } catch {
        // ignore autoplay restrictions
      }
    };
    play();

    return () => {
      if (el.srcObject === stream) {
        el.srcObject = null;
      }
    };
  }, [stream]);

  React.useEffect(() => {
    const onMove = (ev: PointerEvent) => {
      if (!dragRef.current.dragging) return;
      const c = containerRef.current;
      if (!c) return;

      const rect = c.getBoundingClientRect();
      const w = rect.width;
      const h = rect.height;

      const nextX = ev.clientX - dragRef.current.offsetX;
      const nextY = ev.clientY - dragRef.current.offsetY;
      const clampedX = Math.min(Math.max(8, nextX), window.innerWidth - w - 8);
      const clampedY = Math.min(Math.max(8, nextY), window.innerHeight - h - 8);
      setPos({ x: clampedX, y: clampedY });
    };

    const onUp = () => {
      dragRef.current.dragging = false;
    };

    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    return () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };
  }, []);

  const label = activeVideoSource === 'screen' ? 'Screen' : activeVideoSource === 'camera' ? 'Camera' : 'Preview';

  return (
    <div
      ref={containerRef}
      className="fixed z-50 select-none rounded-lg border border-white/20 bg-black/50 shadow-lg backdrop-blur"
      style={{ left: pos.x, top: pos.y, width: 200, maxWidth: 200 }}
      onPointerDown={(ev) => {
        const rect = (ev.currentTarget as HTMLDivElement).getBoundingClientRect();
        dragRef.current.dragging = true;
        dragRef.current.offsetX = ev.clientX - rect.left;
        dragRef.current.offsetY = ev.clientY - rect.top;
      }}
    >
      <div className="flex items-center justify-between px-2 py-1 text-[10px] uppercase tracking-wide text-white/80">
        <span>{label}</span>
        <span className="h-1.5 w-1.5 rounded-full bg-[#5381A5]" />
      </div>

      <video
        ref={videoRef}
        className="block h-[112px] w-full rounded-b-lg object-cover"
        autoPlay
        muted
        playsInline
      />
    </div>
  );
}

