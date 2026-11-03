//! The knowledge-base icon. Kept apart from shared.tsx, which carries the Markdown renderer and sanitizer, so
//! the activity bar and the tab bar can draw it without pulling those into the entry bundle.

export function MemoryIcon({ size = 16 }: { size?: number }) {
  return <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <path d="M12 5v15M3 4.5c3.5-1 6-.5 9 1.5 3-2 5.5-2.5 9-1.5v14c-3.5-1-6-.5-9 1.5-3-2-5.5-2.5-9-1.5z" />
    <path d="M6 8h2m8 0h2M6 12h2m8 0h2" />
  </svg>;
}
