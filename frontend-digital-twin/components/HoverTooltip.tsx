import React, { useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

type Placement = 'top' | 'bottom';

export interface HoverTooltipProps {
  title: string;
  description: string | React.ReactNode;
  children: React.ReactElement;
}

/**
 * Hover tooltip rendered in a portal (document.body) to avoid being clipped by
 * scroll containers/overflow and to ensure it sits above panels.
 */
const HoverTooltip: React.FC<HoverTooltipProps> = ({ title, description, children }) => {
  const anchorRef = useRef<HTMLElement | null>(null);
  const tooltipRef = useRef<HTMLDivElement | null>(null);

  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState<{ top: number; left: number; placement: Placement }>({
    top: 0,
    left: 0,
    placement: 'top',
  });

  const recalc = () => {
    if (!anchorRef.current || !tooltipRef.current) return;

    const margin = 10;
    const viewportPadding = 8;

    const a = anchorRef.current.getBoundingClientRect();
    const t = tooltipRef.current.getBoundingClientRect();

    // Prefer placing above; flip below if insufficient space.
    let placement: Placement = 'top';
    let top = a.top - t.height - margin;
    if (top < viewportPadding) {
      placement = 'bottom';
      top = a.bottom + margin;
    }

    // Center horizontally on the anchor, clamped into viewport.
    let left = a.left + a.width / 2 - t.width / 2;
    const maxLeft = window.innerWidth - t.width - viewportPadding;
    left = Math.max(viewportPadding, Math.min(left, maxLeft));

    setPos({ top, left, placement });
  };

  useLayoutEffect(() => {
    if (!open) return;
    if (!anchorRef.current || !tooltipRef.current) return;

    // Initial layout pass.
    recalc();
  }, [open, title, description]);

  // Reposition on scroll/resize while open.
  useLayoutEffect(() => {
    if (!open) return;
    const onReflow = () => {
      // Recompute on any scroll container + window resize.
      // Use RAF to ensure the layout reflects the latest scroll position.
      requestAnimationFrame(recalc);
    };

    window.addEventListener('scroll', onReflow, true);
    window.addEventListener('resize', onReflow);
    return () => {
      window.removeEventListener('scroll', onReflow, true);
      window.removeEventListener('resize', onReflow);
    };
  }, [open]);

  const child = React.Children.only(children) as React.ReactElement<any>;
  const existingRef = (child as any).ref;

  const setMergedRef = (node: HTMLElement | null) => {
    anchorRef.current = node;
    if (typeof existingRef === 'function') {
      existingRef(node);
    } else if (existingRef && typeof existingRef === 'object') {
      (existingRef as React.MutableRefObject<HTMLElement | null>).current = node;
    }
  };

  const onMouseEnter = (e: React.MouseEvent) => {
    child.props.onMouseEnter?.(e);
    setOpen(true);
  };

  const onMouseLeave = (e: React.MouseEvent) => {
    child.props.onMouseLeave?.(e);
    setOpen(false);
  };

  const onFocus = (e: React.FocusEvent) => {
    child.props.onFocus?.(e);
    setOpen(true);
  };

  const onBlur = (e: React.FocusEvent) => {
    child.props.onBlur?.(e);
    setOpen(false);
  };

  return (
    <>
      {React.cloneElement(child, {
        ref: setMergedRef,
        onMouseEnter,
        onMouseLeave,
        onFocus,
        onBlur,
      })}
      {open && typeof document !== 'undefined'
        ? createPortal(
            <div
              ref={tooltipRef}
              style={{
                position: 'fixed',
                top: pos.top,
                left: pos.left,
                zIndex: 9999,
              }}
              className="pointer-events-none max-w-[280px] rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.35)] bg-[rgb(var(--surface-rgb)/0.9)] p-2 shadow-2xl text-left"
            >
              <div className="text-[10px] font-black text-[var(--bg-steel)] uppercase tracking-widest mb-1">
                {title}
              </div>
              <div className="text-[10px] text-[var(--text-secondary)] leading-snug whitespace-normal break-words">
                {typeof description === 'string' ? description : description}
              </div>

              {/* Arrow */}
              <div
                className="absolute left-1/2 -translate-x-1/2 border-8 border-transparent"
                style={
                  pos.placement === 'top'
                    ? { top: '100%', borderTopColor: 'rgb(var(--bg-steel-rgb) / 0.35)' }
                    : { bottom: '100%', borderBottomColor: 'rgb(var(--bg-steel-rgb) / 0.35)' }
                }
              />
            </div>,
            document.body,
          )
        : null}
    </>
  );
};

export default HoverTooltip;

