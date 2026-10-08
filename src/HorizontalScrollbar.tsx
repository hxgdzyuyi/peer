import {
  forwardRef,
  useImperativeHandle,
  useLayoutEffect,
  useRef,
} from "react";

export type HorizontalScrollbarHandle = { refresh: () => void };

// A virtualized file's own scrollbar can be below the viewport. Keep a native
// scrollbar within reach and mirror only the active file's rendered columns.
export const HorizontalScrollbar = forwardRef<
  HorizontalScrollbarHandle,
  { getHost: () => HTMLElement | undefined; path?: string }
>(function HorizontalScrollbar({ getHost, path }, ref) {
  const bar = useRef<HTMLDivElement>(null);
  const getActiveHost = useRef(getHost);
  getActiveHost.current = getHost;
  const refresh = useRef(() => {});
  useImperativeHandle(ref, () => ({ refresh: () => refresh.current() }), []);

  useLayoutEffect(() => {
    const element = bar.current!;
    const spacer = element.firstElementChild as HTMLElement;
    let host: HTMLElement | undefined;
    let columns: HTMLElement[] = [];
    let source: HTMLElement | undefined;
    let frame: number | undefined;
    let mirroredLeft = 0;
    const schedule = () => {
      frame ??= requestAnimationFrame(measure);
    };
    const observer = new ResizeObserver(schedule);
    function measure() {
      frame = undefined;
      const nextHost = getActiveHost.current();
      // A resize/render frame can run before the native scrollbar's scroll
      // event. Apply that pending user movement instead of resetting the bar
      // from the old code position (or a replaced highlighting DOM node).
      const pendingLeft =
        host === nextHost && Math.abs(element.scrollLeft - mirroredLeft) > 1
          ? element.scrollLeft
          : undefined;
      const nextColumns = Array.from(
        nextHost?.shadowRoot?.querySelectorAll<HTMLElement>("[data-code]") ??
          [],
      );
      if (
        host !== nextHost ||
        nextColumns.some((node, i) => node !== columns[i]) ||
        nextColumns.length !== columns.length
      ) {
        host?.shadowRoot?.removeEventListener("scroll", schedule, true);
        observer.disconnect();
        observer.observe(element);
        host = nextHost;
        columns = nextColumns;
        host?.shadowRoot?.addEventListener("scroll", schedule, {
          capture: true,
          passive: true,
        });
        if (host) observer.observe(host);
        columns.forEach((node) => observer.observe(node));
      }
      // Drive the wider side so a shorter split column cannot truncate its range.
      source = columns.reduce<HTMLElement | undefined>(
        (widest, node) =>
          !widest ||
          node.scrollWidth - node.clientWidth >
            widest.scrollWidth - widest.clientWidth
            ? node
            : widest,
        undefined,
      );
      if (source && pendingLeft !== undefined) source.scrollLeft = pendingLeft;
      const overflow = source ? source.scrollWidth - source.clientWidth : 0;
      spacer.style.width = `${element.clientWidth + overflow}px`;
      element.style.visibility = overflow > 1 ? "visible" : "hidden";
      element.tabIndex = overflow > 1 ? 0 : -1;
      element.setAttribute("aria-hidden", String(overflow <= 1));
      element.scrollLeft = source?.scrollLeft ?? 0;
      mirroredLeft = element.scrollLeft;
    }
    const scroll = () => {
      if (source && Math.abs(source.scrollLeft - element.scrollLeft) > 1)
        source.scrollLeft = element.scrollLeft;
    };
    element.addEventListener("scroll", scroll, { passive: true });
    refresh.current = schedule;
    observer.observe(element);
    schedule();
    return () => {
      refresh.current = () => {};
      if (frame !== undefined) cancelAnimationFrame(frame);
      observer.disconnect();
      host?.shadowRoot?.removeEventListener("scroll", schedule, true);
      element.removeEventListener("scroll", scroll);
    };
  }, []);
  useLayoutEffect(() => refresh.current());

  return (
    <div
      ref={bar}
      className="viewer-horizontal-scrollbar"
      role="region"
      aria-label={`Scroll code horizontally${path ? `: ${path}` : ""}`}
      title="Scroll horizontally · Arrow keys, Home or End"
      onKeyDown={(event) => {
        const element = event.currentTarget;
        const positions: Record<string, number> = {
          ArrowLeft: element.scrollLeft - 40,
          ArrowRight: element.scrollLeft + 40,
          Home: 0,
          End: element.scrollWidth,
          PageUp: element.scrollLeft - element.clientWidth,
          PageDown: element.scrollLeft + element.clientWidth,
        };
        if (event.key in positions) {
          event.preventDefault();
          element.scrollLeft = positions[event.key];
        }
      }}
    >
      <div />
    </div>
  );
});
