import { useCallback, useEffect, useRef, useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  formatBytes,
  formatFileCount,
  formatItemDate,
  formatResolution,
  loadThumb,
  openMedia,
  queryLibrary,
  removeFromLibrary,
  scanLibrary,
  type LibraryItem,
  type LibraryPage,
  type MediaKind,
  type SortKey,
} from "../../api/library";
import { formatDuration } from "../../api/tasks";
import { useI18n } from "../../i18n";

const SORTS: {
  value: SortKey;
  labelKey:
    | "library.sort.dateDesc"
    | "library.sort.dateAsc"
    | "library.sort.title"
    | "library.sort.duration"
    | "library.sort.size"
    | "library.sort.channel";
}[] = [
  { value: "dateDesc", labelKey: "library.sort.dateDesc" },
  { value: "dateAsc", labelKey: "library.sort.dateAsc" },
  { value: "title", labelKey: "library.sort.title" },
  { value: "duration", labelKey: "library.sort.duration" },
  { value: "size", labelKey: "library.sort.size" },
  { value: "channel", labelKey: "library.sort.channel" },
];

const PAGE_SIZE = 200;

function Cover({ item }: { item: LibraryItem }) {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    if (!item.thumbPath) {
      setSrc(null);
      return;
    }
    loadThumb(item.thumbPath)
      .then((data) => {
        if (alive) setSrc(data);
      })
      .catch(() => {
        if (alive) setSrc(null);
      });
    return () => {
      alive = false;
    };
  }, [item.thumbPath]);

  if (src) {
    return <img className="cover" src={src} alt="" loading="lazy" />;
  }
  return (
    <div className="cover cover-empty" aria-hidden="true">
      {item.kind === "audio" ? (
        <svg viewBox="0 0 24 24">
          <path d="M9 18V5l12-2v13" />
          <circle cx="6" cy="18" r="3" />
          <circle cx="18" cy="16" r="3" />
        </svg>
      ) : (
        <svg viewBox="0 0 24 24">
          <rect x="3" y="5" width="18" height="14" rx="2" />
          <path d="m10 9 5 3-5 3z" />
        </svg>
      )}
    </div>
  );
}

function metaLine(item: LibraryItem, language: "ru" | "en"): string {
  return [
    formatBytes(item.sizeBytes, language),
    formatItemDate(item.mtime, language),
    formatResolution(item.width, item.height),
  ]
    .filter(Boolean)
    .join(" · ");
}

export default function LibraryScreen({
  onPractice,
}: {
  onPractice?: (item: LibraryItem) => void;
}) {
  const { language, t } = useI18n();
  const [search, setSearch] = useState("");
  const [kind, setKind] = useState<MediaKind | undefined>(undefined);
  const [sort, setSort] = useState<SortKey>("dateDesc");
  const [view, setView] = useState<"grid" | "list">("grid");
  const [page, setPage] = useState<LibraryPage>({ items: [], total: 0 });
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [menu, setMenu] = useState<{ item: LibraryItem; x: number; y: number } | null>(
    null,
  );
  const [tick, setTick] = useState(0);
  const searchRef = useRef<HTMLInputElement>(null);
  const versionRef = useRef(0);
  const loadingMoreRef = useRef(false);

  const refresh = useCallback((full: boolean) => {
    setScanning(true);
    scanLibrary(full)
      .catch((cause: unknown) => setError(String(cause)))
      .finally(() => {
        setScanning(false);
        setTick((value) => value + 1);
      });
  }, []);

  useEffect(() => {
    refresh(false);
  }, [refresh]);

  useEffect(() => {
    let alive = true;
    const version = ++versionRef.current;
    const run = () => {
      setLoading(true);
      setLoadingMore(false);
      loadingMoreRef.current = false;
      queryLibrary({
        search: search.trim() || undefined,
        kind,
        sort,
        limit: PAGE_SIZE,
        offset: 0,
      })
        .then((result) => {
          if (alive && versionRef.current === version) setPage(result);
        })
        .catch((cause: unknown) => {
          if (alive && versionRef.current === version) setError(String(cause));
        })
        .finally(() => {
          if (alive && versionRef.current === version) setLoading(false);
        });
    };
    const timer = setTimeout(run, search ? 250 : 0);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [search, kind, sort, tick]);

  const loadMore = useCallback(() => {
    if (
      loading ||
      loadingMoreRef.current ||
      page.items.length >= page.total
    ) {
      return;
    }
    const version = versionRef.current;
    loadingMoreRef.current = true;
    setLoadingMore(true);
    queryLibrary({
      search: search.trim() || undefined,
      kind,
      sort,
      limit: PAGE_SIZE,
      offset: page.items.length,
    })
      .then((result) => {
        if (versionRef.current !== version) return;
        setPage((current) => ({
          items: [...current.items, ...result.items],
          total: result.total,
        }));
      })
      .catch((cause: unknown) => {
        if (versionRef.current === version) setError(String(cause));
      })
      .finally(() => {
        if (versionRef.current === version) {
          loadingMoreRef.current = false;
          setLoadingMore(false);
        }
      });
  }, [kind, loading, page.items.length, page.total, search, sort]);

  function onScroll(event: React.UIEvent<HTMLDivElement>) {
    const element = event.currentTarget;
    if (
      element.scrollHeight - element.scrollTop - element.clientHeight <
      300
    ) {
      loadMore();
    }
  }

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "/") {
        event.preventDefault();
        searchRef.current?.focus();
      }
      if (event.key === "Escape") setMenu(null);
    };
    const onClick = () => setMenu(null);
    window.addEventListener("keydown", onKey);
    window.addEventListener("click", onClick);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("click", onClick);
    };
  }, []);

  async function play(item: LibraryItem) {
    try {
      await openMedia(item.path);
    } catch (cause) {
      setError(t("library.openError", { error: String(cause) }));
    }
  }

  function activate(item: LibraryItem) {
    if (item.kind === "audio" && onPractice) {
      onPractice(item);
      return;
    }
    play(item);
  }

  async function reveal(item: LibraryItem) {
    try {
      await revealItemInDir(item.path);
    } catch (cause) {
      setError(String(cause));
    }
  }

  async function remove(item: LibraryItem) {
    try {
      await removeFromLibrary(item.id);
      setPage((current) => ({
        items: current.items.filter((entry) => entry.id !== item.id),
        total: Math.max(0, current.total - 1),
      }));
    } catch (cause) {
      setError(String(cause));
    }
  }

  const empty = !loading && page.items.length === 0;

  return (
    <div className="library">
      <div className="lib-toolbar">
        <div className="search">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <circle cx="11" cy="11" r="7" />
            <path d="m17 17 4 4" />
          </svg>
          <input
            ref={searchRef}
            value={search}
            onChange={(event) => setSearch(event.currentTarget.value)}
            placeholder={t("library.searchPlaceholder")}
            aria-label={t("library.searchLabel")}
          />
        </div>

        <div className="tabs lib-tabs" role="tablist" aria-label={t("library.kindGroup")}>
          {[
            { id: undefined, label: t("library.all") },
            { id: "audio" as MediaKind, label: t("common.audio") },
            { id: "video" as MediaKind, label: t("common.video") },
          ].map((tab) => (
            <button
              key={tab.label}
              type="button"
              role="tab"
              aria-selected={kind === tab.id}
              className={kind === tab.id ? "tab active" : "tab"}
              onClick={() => setKind(tab.id)}
            >
              {tab.label}
            </button>
          ))}
        </div>

        <div className="lib-tools">
          <label className="sort">
            <span className="sr-only">{t("library.sortLabel")}</span>
            <select
              value={sort}
              onChange={(event) => setSort(event.currentTarget.value as SortKey)}
            >
              {SORTS.map((option) => (
                <option key={option.value} value={option.value}>
                  {t(option.labelKey)}
                </option>
              ))}
            </select>
          </label>

          <div className="view-toggle" role="group" aria-label={t("library.viewLabel")}>
            <button
              type="button"
              aria-pressed={view === "grid"}
              aria-label={t("library.grid")}
              className={view === "grid" ? "icon-btn on" : "icon-btn"}
              onClick={() => setView("grid")}
            >
              <svg viewBox="0 0 24 24">
                <rect x="3" y="3" width="7" height="7" rx="1" />
                <rect x="14" y="3" width="7" height="7" rx="1" />
                <rect x="3" y="14" width="7" height="7" rx="1" />
                <rect x="14" y="14" width="7" height="7" rx="1" />
              </svg>
            </button>
            <button
              type="button"
              aria-pressed={view === "list"}
              aria-label={t("library.list")}
              className={view === "list" ? "icon-btn on" : "icon-btn"}
              onClick={() => setView("list")}
            >
              <svg viewBox="0 0 24 24">
                <path d="M4 6h16M4 12h16M4 18h16" />
              </svg>
            </button>
          </div>

          <button
            type="button"
            className={scanning ? "icon-btn scan spinning" : "icon-btn scan"}
            aria-label={t("library.scan")}
            onClick={() => refresh(true)}
            disabled={scanning}
          >
            <svg viewBox="0 0 24 24">
              <path d="M21 12a9 9 0 1 1-3-6.7" />
              <path d="M21 4v5h-5" />
            </svg>
          </button>

          <span className="counter">
            {formatFileCount(page.total, language)}
          </span>
        </div>
      </div>

      {error && (
        <p className="banner" role="alert">
          {error}
        </p>
      )}

      {empty && page.total === 0 && !search && !kind ? (
        <div className="lib-empty">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <rect x="3" y="4" width="18" height="16" rx="2" />
            <path d="M8 4v16" />
          </svg>
          <p>{t("library.empty")}</p>
        </div>
      ) : empty ? (
        <p className="queue-empty">{t("library.noResults")}</p>
      ) : view === "grid" ? (
        <div className="lib-grid" onScroll={onScroll}>
          {page.items.map((item) => (
            <div
              key={item.id}
              className="media-card"
              role="button"
              tabIndex={0}
              onDoubleClick={() => activate(item)}
              onKeyDown={(event) => {
                if (event.key === "Enter") activate(item);
              }}
              onContextMenu={(event) => {
                event.preventDefault();
                setMenu({ item, x: event.clientX, y: event.clientY });
              }}
            >
              <div className="poster">
                <Cover item={item} />
                <span className="dur">
                  {formatDuration(item.durationSecs)}
                </span>
              </div>
              <div className="media-title" title={item.title}>
                {item.title}
              </div>
              <div className="media-sub">
                {item.channel ??
                  (item.kind === "audio"
                    ? t("common.audio")
                    : t("common.video"))}
              </div>
              <div className="media-meta">{metaLine(item, language)}</div>
            </div>
          ))}
        </div>
      ) : (
        <div className="lib-list" onScroll={onScroll}>
          <div className="list-head">
            <span>{t("library.colTitle")}</span>
            <span>{t("library.colType")}</span>
            <span>{t("library.colDuration")}</span>
            <span>{t("library.colSize")}</span>
            <span>{t("library.colDate")}</span>
            <span>{t("library.colQuality")}</span>
          </div>
          {page.items.map((item) => (
            <div
              key={item.id}
              className="list-row"
              role="button"
              tabIndex={0}
              onDoubleClick={() => activate(item)}
              onKeyDown={(event) => {
                if (event.key === "Enter") activate(item);
              }}
              onContextMenu={(event) => {
                event.preventDefault();
                setMenu({ item, x: event.clientX, y: event.clientY });
              }}
            >
              <span className="list-title" title={item.title}>
                {item.title}
              </span>
              <span>
                {item.kind === "audio" ? t("common.audio") : t("common.video")}
              </span>
              <span>{formatDuration(item.durationSecs)}</span>
              <span>{formatBytes(item.sizeBytes, language)}</span>
              <span>{formatItemDate(item.mtime, language)}</span>
              <span>{formatResolution(item.width, item.height) || "—"}</span>
            </div>
          ))}
        </div>
      )}

      {menu && (
        <div
          className="context-menu"
          style={{ left: menu.x, top: menu.y }}
          role="menu"
        >
          <button type="button" role="menuitem" onClick={() => play(menu.item)}>
            {t("library.menu.play")}
          </button>
          {menu.item.kind === "audio" && onPractice && (
            <button
              type="button"
              role="menuitem"
              onClick={() => onPractice(menu.item)}
            >
              {t("library.menu.practice")}
            </button>
          )}
          <button type="button" role="menuitem" onClick={() => reveal(menu.item)}>
            {t("library.menu.reveal")}
          </button>
          <button
            type="button"
            role="menuitem"
            onClick={() => navigator.clipboard?.writeText(menu.item.path)}
          >
            {t("library.menu.copyPath")}
          </button>
          <button type="button" role="menuitem" onClick={() => remove(menu.item)}>
            {t("library.menu.remove")}
          </button>
        </div>
      )}

      <footer className="lib-footer">
        {scanning
          ? t("library.scanning")
          : loadingMore
            ? t("library.loadingMore")
            : t("library.filesCount", { count: page.total })}
      </footer>
    </div>
  );
}
