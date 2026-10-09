import { useState } from "react";
import { defaultPreferences, type LanguageServer } from "./types";

export function NavigationSettings({
  servers,
  onChange,
}: {
  servers: LanguageServer[];
  onChange: (servers: LanguageServer[]) => void;
}) {
  const [draft, setDraft] = useState<LanguageServer[]>(() =>
    structuredClone(servers),
  );
  const [advanced, setAdvanced] = useState<string | null>(null);
  const [error, setError] = useState("");
  const update = (id: string, change: Partial<LanguageServer>) =>
    setDraft((values) =>
      values.map((value) =>
        value.id === id ? { ...value, ...change } : value,
      ),
    );
  const save = () => {
    try {
      const value: LanguageServer[] =
        advanced === null ? draft : JSON.parse(advanced);
      if (!Array.isArray(value))
        throw new Error("Configuration must be an array of language servers.");
      const ids = new Set<string>(),
        extensions = new Set<string>();
      for (const server of value) {
        if (
          !server.id ||
          ids.has(server.id) ||
          typeof server.name !== "string" ||
          typeof server.enabled !== "boolean" ||
          !server.executable?.trim() ||
          !Array.isArray(server.args) ||
          !server.args.every((arg) => typeof arg === "string") ||
          !Array.isArray(server.rootMarkers) ||
          !server.rootMarkers.every(
            (marker) =>
              typeof marker === "string" &&
              !!marker &&
              !marker.startsWith("/") &&
              !marker.split(/[\\/]/).includes(".."),
          ) ||
          !server.languages ||
          typeof server.languages !== "object" ||
          !Object.keys(server.languages).length ||
          !server.env ||
          typeof server.env !== "object" ||
          !Object.values(server.env).every((value) => typeof value === "string")
        )
          throw new Error(
            "Each server needs a unique ID, executable, argument array, language mapping, root markers, and environment object.",
          );
        ids.add(server.id);
        for (const [extension, language] of Object.entries(server.languages)) {
          if (
            !extension ||
            /[.\\/]/.test(extension) ||
            typeof language !== "string" ||
            !language
          )
            throw new Error(
              "Use extensions without a dot and nonempty LSP language IDs.",
            );
          if (server.enabled && extensions.has(extension.toLowerCase()))
            throw new Error(
              `More than one enabled server handles .${extension}.`,
            );
          if (server.enabled) extensions.add(extension.toLowerCase());
        }
      }
      onChange(value);
      setDraft(value);
      setAdvanced(null);
      setError("");
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
    }
  };
  return (
    <section className="navigation-settings">
      <div className="section-heading">
        <div>
          <h3>Code navigation</h3>
          <p>
            F12 goes to definitions. Ctrl+- goes back. Install language servers
            and toolchains locally.
          </p>
        </div>
      </div>
      {advanced === null ? (
        draft.map((server) => (
          <div className="language-server" key={server.id}>
            <label>
              <input
                type="checkbox"
                checked={server.enabled}
                onChange={(event) =>
                  update(server.id, { enabled: event.target.checked })
                }
              />
              {server.name}
            </label>
            <label>
              Executable
              <input
                aria-label={`${server.name} executable`}
                value={server.executable}
                onChange={(event) =>
                  update(server.id, { executable: event.target.value })
                }
              />
            </label>
            <small className="muted">
              {Object.keys(server.languages)
                .map((extension) => `.${extension}`)
                .join(", ")}
              {server.args.length
                ? ` · Arguments: ${JSON.stringify(server.args)}`
                : ""}
            </small>
          </div>
        ))
      ) : (
        <label className="language-server-json">
          Language server configuration (JSON)
          <textarea
            aria-label="Language server configuration"
            spellCheck={false}
            rows={18}
            value={advanced}
            onChange={(event) => setAdvanced(event.target.value)}
          />
        </label>
      )}
      <p className="muted">
        Advanced configuration supports custom servers, extension → language
        mappings, arguments, project root markers, environment,
        initializationOptions, and settings. Commands run directly, without a
        shell. Installed dependency source is not tied to a Git revision.
      </p>
      <p className="muted">
        Presets use existing dependencies without automatic downloads. Rust
        build scripts and procedural macros are disabled by default; generated
        symbols may be unavailable.
      </p>
      {error && (
        <p role="alert" className="remove">
          {error}
        </p>
      )}
      <div className="navigation-settings-actions">
        <button
          className="small-button"
          onClick={() =>
            setAdvanced(
              advanced === null ? JSON.stringify(draft, null, 2) : null,
            )
          }
        >
          {advanced === null
            ? "Advanced / custom servers"
            : "Cancel JSON changes"}
        </button>
        <button
          className="small-button"
          onClick={() => {
            setDraft(structuredClone(defaultPreferences.languageServers));
            setAdvanced(null);
          }}
        >
          Restore presets
        </button>
        <button className="primary-button" onClick={save}>
          Save navigation settings
        </button>
      </div>
    </section>
  );
}
