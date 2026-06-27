import { Component, type ReactNode } from "react";
import { Card } from "./ui";

interface Props { children: ReactNode }
interface State { error: Error | null }

export default class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  render() {
    if (this.state.error) {
      return (
        <div style={{ display: "flex", justifyContent: "center", marginTop: "3rem" }}>
          <Card style={{ textAlign: "center", maxWidth: 420 }}>
            <p style={{ fontWeight: 700, marginBottom: "0.5rem" }}>Something went wrong</p>
            <p className="muted" style={{ fontSize: "0.875rem", marginBottom: "1.25rem" }}>
              {this.state.error.message}
            </p>
            <button className="btn-block" onClick={() => window.location.reload()}>
              Reload page
            </button>
          </Card>
        </div>
      );
    }
    return this.props.children;
  }
}
