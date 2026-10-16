import { describe, expect, it } from "vitest";
import { supportsChatEngine, supportsPermissionToggle, type SessionKind } from "./types";
import { kindLabel, supportsAgentArgs } from "./layout/sessionMenuShared";
import { AGENT_KIND_LABEL } from "./layout/sessionViewers/sessionMeta";

describe("独立 Zcode 子智能体", () => {
  it("在会话界面显示独立名称，并提供对话驱动及配置入口", () => {
    const kind = "zcode" as SessionKind;
    expect(kindLabel(kind)).toBe("Zcode");
    expect(AGENT_KIND_LABEL[kind]).toBe("Zcode");
    expect(supportsChatEngine(kind)).toBe(true);
    expect(supportsAgentArgs(kind)).toBe(true);
    expect(supportsPermissionToggle(kind)).toBe(true);
  });
});
