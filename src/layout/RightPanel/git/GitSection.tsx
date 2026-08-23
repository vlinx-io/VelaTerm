//! Collapsible group header for the Git panel: a disclosure arrow, a title, its count, and the
//! action that applies to the whole group. The action only appears when the group has rows, so an
//! empty group stays a quiet single line.

import Icons from "../../../components/Icons";

export function GitSection({
  title,
  count,
  open,
  onToggle,
  action,
  children,
}: {
  title: string;
  count: number;
  open: boolean;
  onToggle: () => void;
  /** Right-aligned group action, such as Stage all or View all. */
  action?: { label: string; onClick: () => void; disabled?: boolean };
  children?: React.ReactNode;
}) {
  return (
    <div className="git-sec">
      <div className="git-sec-head">
        <button
          type="button"
          className="git-sec-toggle"
          onClick={onToggle}
          aria-expanded={open}
        >
          <span className="tw">{open ? <Icons.chevD size={12} /> : <Icons.chevR size={12} />}</span>
          <span className="ttl">{title}</span>
          <span className="cnt">{count}</span>
        </button>
        {action && count > 0 && (
          <button
            type="button"
            className="git-sec-act"
            onClick={action.onClick}
            disabled={action.disabled}
          >
            {action.label}
          </button>
        )}
      </div>
      {open && children}
    </div>
  );
}
