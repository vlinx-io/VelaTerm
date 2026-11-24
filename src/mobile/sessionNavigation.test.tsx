import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MobileSessionLink, mobileSessionUrl, navigateMobileSession, useMobileSessionNavigation } from "./sessionNavigation";

function Location() {
  const { sessionId, view } = useMobileSessionNavigation();
  return <output>{`${sessionId ?? "list"}:${view}`}</output>;
}
afterEach(() => { cleanup(); history.replaceState(null, "", "/"); });

describe("mobile session URLs", () => {
  it("restores a direct history URL without a preceding navigation", () => {
    history.replaceState(null, "", "/?view=mobile&session=kiro-1&sessionView=history");
    render(<Location />);
    expect(screen.getByText("kiro-1:history")).toBeTruthy();
  });
  it("preserves the connection path and unrelated query/hash while encoding session identity", () => {
    history.replaceState(null, "", "/share/project?view=mobile&filter=active#anchor");
    expect(mobileSessionUrl("id +&", "history")).toBe(`${location.origin}/share/project?view=mobile&filter=active&session=id+%2B%26&sessionView=history#anchor`);
  });
  it("provides actual terminal/history link targets and updates the mounted view", () => {
    render(<><Location /><MobileSessionLink sessionId="kiro-1" view="history">History</MobileSessionLink></>);
    const link = screen.getByRole("link");
    expect(link.getAttribute("href")).toBe(`${location.origin}/?session=kiro-1&sessionView=history`);
    fireEvent.click(link);
    expect(screen.getByText("kiro-1:history")).toBeTruthy();
    act(() => navigateMobileSession("kiro-1"));
    expect(screen.getByText("kiro-1:terminal")).toBeTruthy();
    expect(location.search).toBe("?session=kiro-1");
  });
  it("restores history and terminal modes through browser back and forward", async () => {
    history.replaceState(null, "", "/?session=kiro-1");
    render(<Location />);
    act(() => navigateMobileSession("kiro-1", "history"));
    act(() => history.back());
    await waitFor(() => expect(screen.getByText("kiro-1:terminal")).toBeTruthy());
    act(() => history.forward());
    await waitFor(() => expect(screen.getByText("kiro-1:history")).toBeTruthy());
  });
  it("drops the old view when navigating to another session or the list", () => {
    history.replaceState(null, "", "/?session=old&sessionView=history&view=mobile");
    act(() => navigateMobileSession("new"));
    expect(location.search).toBe("?session=new&view=mobile");
    act(() => navigateMobileSession(null));
    expect(location.search).toBe("?view=mobile");
  });
  it("does not add duplicate history entries when reopening the current view", () => {
    history.replaceState(null, "", "/?session=kiro-1&sessionView=history");
    const length = history.length;
    act(() => navigateMobileSession("kiro-1", "history"));
    expect(history.length).toBe(length);
  });
});
