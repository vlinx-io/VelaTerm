import { safeError } from "../ipc/diagnosticSafety";
import { Component, type ErrorInfo, type ReactNode } from "react";
import { t } from "../i18n";

interface Props {
  children: ReactNode;
  fallback?: (error: Error, retry: () => void) => ReactNode;
}

interface State {
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, _info: ErrorInfo) {
    console.error("[VelaTerm] React render crash:", safeError(error));
  }

  render() {
    if (!this.state.error) return this.props.children;
    if (this.props.fallback) return this.props.fallback(this.state.error, () => this.setState({ error: null }));
    return <CrashScreen error={this.state.error} />;
  }
}

function CrashScreen({ error }: { error: Error }) {
  const native = (window as { __VELATERM_CONNECTION_MENU__?: boolean }).__VELATERM_CONNECTION_MENU__;
  return (
    <div style={containerStyle}>
      <div style={cardStyle}>
        <div style={{ fontSize: 15, fontWeight: 700, marginBottom: 6 }}>
          {t("err.renderTitle")}
        </div>
        <div style={{ fontSize: 12, color: "var(--text-dim, var(--text))", marginBottom: 14 }}>
          {t("err.renderDesc")}
        </div>

        <div style={msgStyle}>{error.message}</div>

        {error.stack && (
          <pre style={stackStyle}>{cleanStack(error.stack)}</pre>
        )}

        <button onClick={() => location.reload()} style={btnStyle}>
          {t("err.reload")}
        </button>
        {(native || (window.innerWidth < 768 && history.length > 1)) && <button onClick={() => native ? location.assign("velaterm-ui://close") : history.back()} style={{...btnStyle, marginLeft: 12, minHeight: 44}}>{t("mobile.back")}</button>}
      </div>
    </div>
  );
}

function cleanStack(stack: string): string {
  return stack
    .split("\n")
    .filter((l) => !l.includes("node_modules"))
    .slice(0, 15)
    .join("\n");
}

const containerStyle: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  display: "grid",
  placeItems: "center",
  background: "var(--bg-0)",
  color: "var(--text)",
  fontFamily: "var(--font-ui)",
  zIndex: 99999,
};

const cardStyle: React.CSSProperties = {
  maxWidth: 560,
  width: "90%",
  padding: 28,
  background: "var(--bg-2, color-mix(in srgb, var(--bg-0) 93%, var(--text)))",
  border: "1px solid var(--border-strong, color-mix(in srgb, var(--text) 30%, var(--bg-0)))",
  borderRadius: 12,
};

const msgStyle: React.CSSProperties = {
  fontFamily: "var(--font-mono)",
  padding: "10px 12px",
  background: "color-mix(in srgb, var(--red, var(--text)) 12%, var(--bg-0))",
  border: "1px solid var(--red, var(--text))",
  borderRadius: 6,
  color: "var(--red, var(--text))",
  fontSize: 12.5,
  lineHeight: 1.5,
  wordBreak: "break-word",
  marginBottom: 10,
};

const stackStyle: React.CSSProperties = {
  fontFamily: "var(--font-mono)",
  margin: 0,
  marginBottom: 16,
  padding: "10px 12px",
  background: "var(--bg-0)",
  border: "1px solid var(--border, color-mix(in srgb, var(--text) 30%, var(--bg-0)))",
  borderRadius: 6,
  fontSize: 11,
  lineHeight: 1.5,
  color: "var(--text-mid, var(--text))",
  overflow: "auto",
  maxHeight: 200,
  whiteSpace: "pre-wrap",
  wordBreak: "break-all",
};

const btnStyle: React.CSSProperties = {
  padding: "8px 20px",
  border: "none",
  borderRadius: 6,
  background: "var(--accent)",
  color: "var(--text-on-accent)",
  fontSize: 13,
  fontWeight: 600,
  cursor: "pointer",
};
