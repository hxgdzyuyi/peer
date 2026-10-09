import { useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Modal } from "./Modal";

export function RepositoryLocation({
  path,
  onSave,
  onClose,
}: {
  path: string;
  onSave: (path: string) => Promise<void>;
  onClose: () => void;
}) {
  const [location, setLocation] = useState(path);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const input = useRef<HTMLInputElement>(null);
  const report = (error: unknown) =>
    setError(error instanceof Error ? error.message : String(error));
  return (
    <Modal
      title="Update repository location"
      initialFocus={input}
      onClose={() => {
        if (!busy) onClose();
      }}
    >
      <form
        className="repository-location"
        onSubmit={async (event) => {
          event.preventDefault();
          if (busy || !location.trim()) return;
          setBusy(true);
          setError("");
          try {
            await onSave(location);
          } catch (error) {
            report(error);
          } finally {
            setBusy(false);
          }
        }}
      >
        <p className="settings-help">
          If this repository was renamed or moved, choose its new root folder.
          Peer updates the saved name and path and keeps its review data. This
          does not move or rename files.
        </p>
        <label htmlFor="repository-location">Repository location</label>
        <div className="repository-location-input">
          <input
            id="repository-location"
            ref={input}
            value={location}
            disabled={busy}
            onChange={(event) => setLocation(event.target.value)}
          />
          <button
            type="button"
            disabled={busy}
            onClick={async () => {
              setBusy(true);
              setError("");
              try {
                const selected = await open({
                  directory: true,
                  multiple: false,
                  title: "Locate renamed or moved repository",
                });
                if (typeof selected === "string") setLocation(selected);
              } catch (error) {
                report(error);
              } finally {
                setBusy(false);
              }
            }}
          >
            Browse…
          </button>
        </div>
        {error && (
          <p className="error-banner" role="alert">
            {error}
          </p>
        )}
        <div className="repository-location-actions">
          <button type="button" disabled={busy} onClick={onClose}>
            Cancel
          </button>
          <button
            className="primary-button"
            disabled={busy || !location.trim()}
          >
            {busy ? "Updating…" : "Update location"}
          </button>
        </div>
      </form>
    </Modal>
  );
}
