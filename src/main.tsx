import React from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles.css";

if (import.meta.env.VITE_DESKTOP_TEST === "1")
  await import("@wdio/tauri-plugin");

class ErrorBoundary extends React.Component<
  React.PropsWithChildren,
  { error: string | null }
> {
  state = { error: null as string | null };
  static getDerivedStateFromError(error: Error) {
    return { error: error.message };
  }
  render() {
    return this.state.error ? (
      <main className="fatal">
        <h1>Something interrupted the viewer</h1>
        <p>{this.state.error}</p>
        <button onClick={() => location.reload()}>Reload Peer</button>
        <p>Saved feedback is kept on disk.</p>
      </main>
    ) : (
      this.props.children
    );
  }
}
createRoot(document.getElementById("root")!).render(
  <ErrorBoundary>
    <App />
  </ErrorBoundary>,
);
