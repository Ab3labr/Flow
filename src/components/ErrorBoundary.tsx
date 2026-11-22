import { Component, Fragment, type ErrorInfo, type ReactNode } from "react";
import { noteService } from "../services";
import { logWebview } from "../services/uiService";

/** Smallest valid notes height (see docs/LAYOUT.md), used while a view is down. */
export const ERROR_FALLBACK_HEIGHT = 172;

type Props = {
  /** Display name used in the fallback message and the log line. */
  tab: string;
  children?: ReactNode;
  /** Notes only: lets the boundary restore a valid panel height. */
  onHeightChange?: (height: number) => void;
  /** Notes only: exposes the native "Open notes folder" action. */
  showOpenNotesFolder?: boolean;
};

type State = {
  error: Error | null;
  /** Remount children after "Try again" without remounting the boundary. */
  attempt: number;
};

/**
 * Keeps one broken tab from taking down the whole notch.
 *
 * Only the error message and React's component stack are logged. No note text,
 * file contents or folder paths are ever included (docs/PRIVACY.md).
 */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null, attempt: 0 };

  static getDerivedStateFromError(error: unknown): Partial<State> {
    return { error: error instanceof Error ? error : new Error(String(error)) };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    const message = error instanceof Error ? error.message : String(error);
    const stack = info?.componentStack ?? "none";
    logWebview(
      `[ErrorBoundary] ${this.props.tab}: ${message}; componentStack=${stack}`,
      5,
    );
    // A crashed view may have reported a bad height before it died.
    this.props.onHeightChange?.(ERROR_FALLBACK_HEIGHT);
  }

  private handleRetry = () => {
    this.props.onHeightChange?.(ERROR_FALLBACK_HEIGHT);
    this.setState((state) => ({ error: null, attempt: state.attempt + 1 }));
  };

  private handleOpenNotesFolder = () => {
    const open = (noteService as { openFolder?: () => Promise<void> | void }).openFolder;
    if (typeof open === "function") {
      void Promise.resolve(open.call(noteService)).catch(() => undefined);
    }
  };

  render() {
    const { error, attempt } = this.state;
    const { tab, children, showOpenNotesFolder } = this.props;

    if (error) {
      return (
        <div
          role="alert"
          className="flex h-[120px] flex-col items-center justify-center gap-2 rounded-[14px] bg-[#121212] px-3 text-center"
        >
          <p className="text-[12px] text-white/70">Something went wrong in {tab}</p>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={this.handleRetry}
              className="h-6 rounded-md bg-white/[0.08] px-2 text-[11px] text-white/70 hover:bg-white/[0.12]"
            >
              Try again
            </button>
            {showOpenNotesFolder && (
              <button
                type="button"
                onClick={this.handleOpenNotesFolder}
                className="h-6 rounded-md bg-white/[0.08] px-2 text-[11px] text-white/70 hover:bg-white/[0.12]"
              >
                Open notes folder
              </button>
            )}
          </div>
        </div>
      );
    }

    // Changing the key on retry forces a fresh mount of the failed view.
    return <Fragment key={attempt}>{children}</Fragment>;
  }
}
