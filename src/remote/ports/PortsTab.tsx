import { useEffect, useState } from "react";
import { useT } from "../../i18n";
import { Section } from "../../layout/RightPanel/parts";
import { requestPorts, usePorts } from "./portsClient";
import "./ports.css";

export const POLL_MS = 5000;

export function parsePort(text: string): number | null {
  const s = text.trim();
  if (!/^\d{1,5}$/.test(s)) return null;
  const n = Number(s);
  return n >= 1 && n <= 65535 ? n : null;
}

export function PortsTab() {
  const t = useT();
  const { snapshot, unavailable, error } = usePorts();
  const [manual, setManual] = useState("");

  // Detection runs only while the tab is mounted.
  useEffect(() => {
    void requestPorts("list");
    const timer = setInterval(() => void requestPorts("list"), POLL_MS);
    return () => clearInterval(timer);
  }, []);

  if (unavailable) {
    return <div className="insp-section ports-hint">{t("ports.unavailable")}</div>;
  }

  const forwards = snapshot?.forwards ?? [];
  const manualPort = parsePort(manual);
  const submitManual = (event: React.FormEvent) => {
    event.preventDefault();
    if (manualPort === null) return;
    void requestPorts("forward", manualPort);
    setManual("");
  };

  return (
    <>
      {error && <div role="alert" className="ports-error">{error}</div>}
      <Section id="ports-active" title={t("ports.active")} tag={forwards.length || undefined}>
        {forwards.length === 0 ? (
          <div className="ports-hint">{t("ports.activeEmpty")}</div>
        ) : (
          forwards.map((f) => {
            const moved = f.lport !== f.rport;
            return (
              <div className="ports-row" key={f.rport}>
                <span className="port">{f.rport}</span>
                <span
                  className={"local" + (moved ? " accent" : "")}
                  title={moved ? t("ports.localChanged", f.rport) : undefined}
                >
                  localhost:{f.lport}
                </span>
                <button
                  className="ports-btn"
                  aria-label={`${t("ports.open")} ${f.rport}`}
                  onClick={() => void requestPorts("open", f.rport)}
                >
                  {t("ports.open")}
                </button>
                <button
                  className="ports-btn"
                  aria-label={`${t("ports.stop")} ${f.rport}`}
                  onClick={() => void requestPorts("unforward", f.rport)}
                >
                  {t("ports.stop")}
                </button>
              </div>
            );
          })
        )}
      </Section>
      <Section id="ports-detected" title={t("ports.detected")}>
        {snapshot && !snapshot.detectionAvailable ? (
          <div className="ports-hint">{t("ports.detectionUnavailable")}</div>
        ) : snapshot && snapshot.detected.length === 0 ? (
          <div className="ports-hint">{t("ports.detectedEmpty")}</div>
        ) : (
          snapshot?.detected.map((port) => (
            <div className="ports-row" key={port}>
              <span className="port">{port}</span>
              <span className="sp" />
              <button
                className="ports-btn"
                aria-label={`${t("ports.forward")} ${port}`}
                onClick={() => void requestPorts("forward", port)}
              >
                {t("ports.forward")}
              </button>
            </div>
          ))
        )}
      </Section>
      <Section id="ports-manual" title={t("ports.manual")}>
        <form className="ports-row" onSubmit={submitManual}>
          <input
            className="ports-input"
            inputMode="numeric"
            aria-label={t("ports.portPlaceholder")}
            placeholder={t("ports.portPlaceholder")}
            value={manual}
            onChange={(event) => setManual(event.target.value)}
          />
          <button className="ports-btn" type="submit" disabled={manualPort === null}>
            {t("ports.forward")}
          </button>
        </form>
      </Section>
    </>
  );
}
