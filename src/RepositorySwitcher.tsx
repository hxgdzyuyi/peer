import { useEffect, useId, useMemo, useRef, useState } from "react";
import { Check, ChevronDown, FolderOpen, Plus } from "lucide-react";

export function RepositorySwitcher({
  current,
  repositories,
  disabled,
  onSelect,
  onOpen,
}: {
  current: { root: string; name: string } | null;
  repositories: string[];
  disabled: boolean;
  onSelect: (path: string) => void;
  onOpen: () => void;
}) {
  const [open, setOpen] = useState(false);
  const menuId = useId();
  const container = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const items = useRef<(HTMLButtonElement | null)[]>([]);
  const initialFocus = useRef(0);
  const paths = useMemo(
    () => [...new Set([...(current ? [current.root] : []), ...repositories])],
    [current?.root, repositories],
  );

  useEffect(() => {
    if (!open) return;
    items.current[initialFocus.current]?.focus();
    const outside = (event: MouseEvent) => {
      if (!container.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("click", outside);
    return () => document.removeEventListener("click", outside);
  }, [open]);
  useEffect(() => {
    if (disabled) setOpen(false);
  }, [disabled]);

  return (
    <div
      ref={container}
      className="repository-switcher"
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) setOpen(false);
      }}
      onKeyDown={(event) => {
        if (open && (event.key === "Escape" || event.key === "Tab")) {
          if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
          }
          setOpen(false);
          trigger.current?.focus();
        }
      }}
    >
      <button
        ref={trigger}
        className="repository-button"
        aria-label="Switch repository"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        title={current?.root ?? "Switch repository"}
        disabled={disabled || !paths.length}
        onClick={() => {
          initialFocus.current = 0;
          setOpen((value) => !value);
        }}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown" || event.key === "ArrowUp") {
            event.preventDefault();
            initialFocus.current =
              event.key === "ArrowUp" ? paths.length - 1 : 0;
            setOpen(true);
          }
        }}
      >
        <FolderOpen size={17} />
        <span>{current?.name ?? "Repositories"}</span>
        <ChevronDown size={13} />
      </button>
      <button
        className="icon-button repository-add"
        aria-label="Open repository"
        title="Open another repository (⌘ O)"
        disabled={disabled}
        onClick={() => {
          setOpen(false);
          onOpen();
        }}
      >
        <Plus size={16} />
      </button>
      {open && (
        <div
          id={menuId}
          className="repository-menu"
          role="menu"
          aria-label="Repositories"
        >
          <div className="repository-menu-label">Repositories</div>
          {paths.map((path, index) => (
            <button
              key={path}
              ref={(node) => {
                items.current[index] = node;
              }}
              role="menuitemradio"
              aria-checked={path === current?.root}
              tabIndex={-1}
              title={path}
              data-repository-path={path}
              onClick={() => {
                setOpen(false);
                trigger.current?.focus();
                if (path !== current?.root) onSelect(path);
              }}
              onKeyDown={(event) => {
                let next: number;
                if (event.key === "ArrowDown")
                  next = (index + 1) % paths.length;
                else if (event.key === "ArrowUp")
                  next = (index - 1 + paths.length) % paths.length;
                else if (event.key === "Home") next = 0;
                else if (event.key === "End") next = paths.length - 1;
                else return;
                event.preventDefault();
                event.stopPropagation();
                items.current[next]?.focus();
              }}
            >
              <FolderOpen size={16} />
              <span className="repository-menu-text">
                <strong>
                  {path.split(/[\\/]/).filter(Boolean).at(-1) ?? path}
                </strong>
                <small>{path}</small>
              </span>
              {path === current?.root && (
                <Check size={15} className="repository-current" />
              )}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
