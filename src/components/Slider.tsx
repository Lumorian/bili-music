import { useCallback, useRef, useState } from 'react';
import { clamp } from '../lib/format';

interface Props {
  value: number;
  max: number;
  onChange?: (v: number) => void;
  onCommit?: (v: number) => void;
  className?: string;
  disabled?: boolean;
}

/** 轻量自绘进度条：桌面用鼠标、安卓用触摸都能拖 */
export function Slider({ value, max, onChange, onCommit, className, disabled }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [dragging, setDragging] = useState(false);
  const [dragValue, setDragValue] = useState(0);

  const pct = max > 0 ? clamp(((dragging ? dragValue : value) / max) * 100, 0, 100) : 0;

  const posOf = useCallback(
    (clientX: number) => {
      const box = ref.current?.getBoundingClientRect();
      if (!box || box.width === 0) return 0;
      return clamp((clientX - box.left) / box.width, 0, 1) * max;
    },
    [max],
  );

  const onPointerDown = (e: React.PointerEvent) => {
    if (disabled || max <= 0) return;
    e.currentTarget.setPointerCapture?.(e.pointerId);
    const v = posOf(e.clientX);
    setDragging(true);
    setDragValue(v);
    onChange?.(v);
  };

  const onPointerMove = (e: React.PointerEvent) => {
    if (!dragging || disabled) return;
    const v = posOf(e.clientX);
    setDragValue(v);
    onChange?.(v);
  };

  const finish = (e: React.PointerEvent) => {
    if (!dragging) return;
    const v = posOf(e.clientX);
    setDragging(false);
    onCommit?.(v);
  };

  return (
    <div
      ref={ref}
      className={`slider ${className ?? ''}`}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={finish}
      onPointerCancel={finish}
      style={disabled ? { opacity: 0.5, cursor: 'default' } : undefined}
      role="slider"
      aria-valuemin={0}
      aria-valuemax={max}
      aria-valuenow={Math.round(value)}
    >
      <div className="slider-fill" style={{ width: `${pct}%` }} />
      <div className="slider-thumb" style={{ left: `${pct}%` }} />
    </div>
  );
}
