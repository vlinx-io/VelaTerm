//! Shared agent picker with the standard dropdown behavior and provider icons.

import { useT } from "../i18n";
import type { SessionKind } from "../types";
import { kindIconEl } from "../layout/sessionViewers/sessionMeta";
import Select from "./Select";

export interface AgentSelectOption { id: SessionKind; label: string; available?: boolean }

export default function AgentSelect({ value, onChange, options, disabled, width = "100%", align = "left", placeholder, ariaLabel }: {
  value: SessionKind | "";
  onChange: (value: SessionKind | "") => void;
  options: AgentSelectOption[];
  disabled?: boolean;
  width?: number | string;
  align?: "left" | "right";
  placeholder?: string;
  /** Accessible name when the visible label is more specific than "Agent". */
  ariaLabel?: string;
}) {
  const t = useT();
  return <Select value={value} onChange={onChange} disabled={disabled} width={width} align={align}
    menuPortal ariaLabel={ariaLabel ?? t("orch.agentLabel")} placeholder={placeholder}
    options={options.map(option => ({ value: option.id, label: option.label,
      disabled: option.available === false, hint: option.available === false ? t("memory.unavailable") : undefined,
      icon: <span aria-hidden="true" style={{ display: "inline-flex", flex: "none" }}>{kindIconEl(option.id, 15)}</span>,
    }))} />;
}
