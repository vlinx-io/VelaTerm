//! Draggable divider that reports the pixel delta for each mouse movement.

import { useT } from "../i18n";

interface SplitterProps {
  onDrag: (deltaX: number) => void;
  onDragStart?: () => void;
  onDragEnd?: (totalDeltaX: number) => void;
}

export function Splitter({ onDrag, onDragStart, onDragEnd }: SplitterProps) {
  const t = useT();
  const handleMouseDown = (e: React.MouseEvent) => {
    e.preventDefault();
    let last = e.clientX;
    const start = e.clientX;
    // A preview iframe takes the pointer as soon as the cursor crosses it, and window mousemove then
    // stops arriving: the drag would freeze mid-way. `splitting` makes those elements transparent
    // to the pointer for the duration of the drag.
    document.body.classList.add("splitting");
    document.body.style.cursor = "col-resize";
    onDragStart?.();
    const onMove = (ev: MouseEvent) => {
      onDrag(ev.clientX - last);
      last = ev.clientX;
    };
    const onUp = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      window.removeEventListener("blur", onUp);
      document.body.classList.remove("splitting");
      document.body.style.cursor = "";
      onDragEnd?.(last - start);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    // Releasing outside the window fires no mouseup here; without this the page would stay in
    // `splitting` and its previews would keep ignoring the pointer.
    window.addEventListener("blur", onUp);
  };

  return (
    <div
      className="vlx-splitter"
      onMouseDown={handleMouseDown}
      style={{
        width: 5,
        flex: "0 0 auto",
        cursor: "col-resize",
        background: "transparent",
        position: "relative",
        zIndex: 5,
      }}
      title={t("splitter.dragToResize")}
    >
      <div
        style={{
          position: "absolute",
          left: 2,
          top: 0,
          bottom: 0,
          width: 1,
          background: "var(--border)",
        }}
      />
    </div>
  );
}
