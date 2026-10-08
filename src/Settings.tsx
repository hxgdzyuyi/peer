import { useState, type CSSProperties } from "react";
import {
  Check,
  Download,
  Moon,
  Search,
  Sun,
  Monitor,
  Trash2,
} from "lucide-react";
import type { Preferences } from "./types";
import { themeVariables, type Theme } from "./themes";
import { Modal } from "./Modal";
export function Settings({
  preferences,
  themes,
  onChange,
  onImport,
  onRemove,
  onClose,
}: {
  preferences: Preferences;
  themes: Theme[];
  onChange: (preferences: Preferences) => void;
  onImport: () => void;
  onRemove: (id: string) => void;
  onClose: () => void;
}) {
  const [filter, setFilter] = useState("");
  const select = (theme: Theme) =>
    onChange({
      ...preferences,
      appearance: {
        ...preferences.appearance,
        mode: theme.kind,
        [theme.kind === "light" ? "lightTheme" : "darkTheme"]: theme.id,
      },
    });
  return (
    <Modal title="Settings" onClose={onClose} className="settings-modal">
      <div className="settings-body">
        <div className="section-heading">
          <div>
            <h3>Appearance</h3>
            <p>Your workspace, in your colors.</p>
          </div>
          <button className="small-button" onClick={onImport}>
            <Download size={14} />
            Import theme
          </button>
        </div>
        <div className="appearance-modes">
          {(["system", "light", "dark"] as const).map((mode) => (
            <button
              key={mode}
              className={preferences.appearance.mode === mode ? "active" : ""}
              onClick={() =>
                onChange({
                  ...preferences,
                  appearance: { ...preferences.appearance, mode },
                })
              }
            >
              {mode === "system" ? (
                <Monitor size={16} />
              ) : mode === "light" ? (
                <Sun size={16} />
              ) : (
                <Moon size={16} />
              )}
              <span>{mode[0].toUpperCase() + mode.slice(1)}</span>
            </button>
          ))}
        </div>
        <div className="theme-pair">
          <label>
            Light theme
            <select
              value={preferences.appearance.lightTheme}
              onChange={(e) =>
                onChange({
                  ...preferences,
                  appearance: {
                    ...preferences.appearance,
                    lightTheme: e.target.value,
                  },
                })
              }
            >
              {themes
                .filter((t) => t.kind === "light")
                .map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.name}
                  </option>
                ))}
            </select>
          </label>
          <label>
            Dark theme
            <select
              value={preferences.appearance.darkTheme}
              onChange={(e) =>
                onChange({
                  ...preferences,
                  appearance: {
                    ...preferences.appearance,
                    darkTheme: e.target.value,
                  },
                })
              }
            >
              {themes
                .filter((t) => t.kind === "dark")
                .map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.name}
                  </option>
                ))}
            </select>
          </label>
        </div>
        <label className="filter-input theme-filter">
          <Search size={14} />
          <input
            aria-label="Search themes"
            placeholder="Search themes…"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
          />
        </label>
        <div className="theme-grid">
          {themes
            .filter((t) => t.name.toLowerCase().includes(filter.toLowerCase()))
            .map((theme) => (
              <div className="theme-tile" key={theme.id}>
                <button
                  onClick={() => select(theme)}
                  aria-label={`Use ${theme.name}`}
                  className={
                    theme.id === preferences.appearance.lightTheme ||
                    theme.id === preferences.appearance.darkTheme
                      ? "chosen"
                      : ""
                  }
                >
                  <div
                    className="theme-preview"
                    style={themeVariables(theme) as CSSProperties}
                  >
                    <div className="mini-sidebar">
                      <i />
                      <i />
                      <i />
                    </div>
                    <div className="mini-code">
                      <span className="muted">// inspect with context</span>
                      <div>
                        <b>const</b> review = {"{"}
                      </div>
                      <div>
                        {" "}
                        <em>ready</em>: <b>true</b>,
                      </div>
                      <div>{"}"}</div>
                      <div className="mini-added">+ share(feedback)</div>
                    </div>
                  </div>
                  <div className="theme-name">
                    <span>{theme.name}</span>
                    {(theme.id === preferences.appearance.lightTheme ||
                      theme.id === preferences.appearance.darkTheme) && (
                      <Check size={14} />
                    )}
                  </div>
                </button>
                {!theme.builtin && (
                  <button
                    className="remove-theme icon-button"
                    aria-label={`Remove ${theme.name}`}
                    onClick={() => onRemove(theme.id)}
                  >
                    <Trash2 size={12} />
                  </button>
                )}
              </div>
            ))}
        </div>
        <p className="settings-help">
          Select a theme to preview it across the app. Import self-contained VS
          Code JSON or JSONC color themes.
        </p>
        <div className="settings-divider" />
        <h3>Viewer</h3>
        <div className="setting-row">
          <label htmlFor="code-font-size">
            Code font size
            <small>Applies immediately to Code and Changes.</small>
          </label>
          <select
            id="code-font-size"
            value={preferences.codeFontSize}
            onChange={(e) =>
              onChange({ ...preferences, codeFontSize: Number(e.target.value) })
            }
          >
            {Array.from({ length: 23 }, (_, i) => i + 10).map((size) => (
              <option key={size} value={size}>
                {size} px
              </option>
            ))}
          </select>
        </div>
        <div className="setting-row">
          <label htmlFor="tab-width">
            Default tab width<small>EditorConfig takes precedence.</small>
          </label>
          <input
            id="tab-width"
            type="number"
            min={1}
            max={16}
            value={preferences.tabWidth}
            onChange={(e) =>
              onChange({
                ...preferences,
                tabWidth: Math.min(
                  16,
                  Math.max(1, Number(e.target.value) || 4),
                ),
              })
            }
          />
        </div>
        <label className="setting-row">
          <span>
            Show whitespace markers
            <small>Preserve original spaces and tabs.</small>
          </span>
          <input
            type="checkbox"
            checked={preferences.showWhitespace}
            onChange={(e) =>
              onChange({ ...preferences, showWhitespace: e.target.checked })
            }
          />
        </label>
        <div className="settings-divider" />
        <h3>Git</h3>
        <label className="git-path">
          Executable path
          <input
            aria-label="Git executable path"
            placeholder="Auto-detect Git"
            value={preferences.gitExecutable ?? ""}
            onChange={(e) =>
              onChange({
                ...preferences,
                gitExecutable: e.target.value || null,
              })
            }
          />
        </label>
        <p className="settings-help">
          Leave empty to use your installed Git. Changes take effect on the next
          repository refresh.
        </p>
      </div>
    </Modal>
  );
}
