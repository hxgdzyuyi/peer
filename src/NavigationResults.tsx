import { X } from "lucide-react";
import type { NavigationLocation, NavigationResult } from "./types";

export function NavigationResults({
  result,
  onSelect,
  onClose,
  onFeedback,
}: {
  result: NavigationResult;
  onSelect: (location: NavigationLocation) => void;
  onClose: () => void;
  onFeedback: () => void;
}) {
  const groups = new Map<string, NavigationLocation[]>();
  for (const location of result.locations) {
    const group = groups.get(location.path) ?? [];
    group.push(location);
    groups.set(location.path, group);
  }
  return (
    <aside
      id="right-sidebar"
      className="navigation-results"
      aria-label={result.kind === "references" ? "References" : "Definitions"}
    >
      <div className="navigation-results-heading">
        <strong>
          {result.kind === "references" ? "References" : "Definitions"} ·{" "}
          {result.locations.length}
        </strong>
        <button
          className="icon-button"
          aria-label="Close navigation results"
          onClick={onClose}
        >
          <X size={14} />
        </button>
      </div>
      <button className="small-button" onClick={onFeedback}>
        Show Feedback
      </button>
      <div className="navigation-result-list">
        {!groups.size && <p className="muted">No {result.kind} found.</p>}
        {[...groups].map(([path, locations]) => (
          <section key={path}>
            <h4 title={path}>{path}</h4>
            {locations.map((location, index) => (
              <button
                key={`${location.documentId}:${index}`}
                className="navigation-result"
                onClick={() => onSelect(location)}
              >
                <span>
                  {location.label} · {location.range.start.line + 1}:
                  {location.range.start.character + 1}
                </span>
                <code>{location.preview || "Open source"}</code>
              </button>
            ))}
          </section>
        ))}
      </div>
    </aside>
  );
}
