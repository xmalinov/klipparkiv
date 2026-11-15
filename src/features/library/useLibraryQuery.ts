import { useCallback, useEffect, useRef, useState } from "react";
import {
  queryLibrary,
  removeFromLibrary,
  type LibraryItem,
  type LibraryPage,
  type MediaKind,
  type SortKey,
} from "../../api/library";
import { errorMessage } from "../../lib/errors";

const PAGE_SIZE = 200;
const SEARCH_DEBOUNCE_MS = 250;

/**
 * Loading, pagination and filtering for the library grid.
 *
 * Extracted from `LibraryScreen` so the screen only renders: the query state
 * (search, kind, sort, page) and the race protection live here and are covered
 * through the screen's behaviour tests.
 */
export function useLibraryQuery() {
  const [search, setSearch] = useState("");
  const [kind, setKind] = useState<MediaKind | undefined>(undefined);
  const [sort, setSort] = useState<SortKey>("dateDesc");
  const [page, setPage] = useState<LibraryPage>({ items: [], total: 0 });
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  const versionRef = useRef(0);
  const loadingMoreRef = useRef(false);

  const reload = useCallback(() => {
    setTick((value) => value + 1);
  }, []);

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
          if (alive && versionRef.current === version) {
            setError(errorMessage(cause));
          }
        })
        .finally(() => {
          if (alive && versionRef.current === version) setLoading(false);
        });
    };
    const timer = setTimeout(run, search ? SEARCH_DEBOUNCE_MS : 0);
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
        if (versionRef.current === version) setError(errorMessage(cause));
      })
      .finally(() => {
        if (versionRef.current === version) {
          loadingMoreRef.current = false;
          setLoadingMore(false);
        }
      });
  }, [kind, loading, page.items.length, page.total, search, sort]);

  const removeItem = useCallback(async (item: LibraryItem) => {
    try {
      await removeFromLibrary(item.id);
      setPage((current) => ({
        items: current.items.filter((entry) => entry.id !== item.id),
        total: Math.max(0, current.total - 1),
      }));
    } catch (cause) {
      setError(errorMessage(cause));
    }
  }, []);

  return {
    search,
    setSearch,
    kind,
    setKind,
    sort,
    setSort,
    page,
    loading,
    loadingMore,
    error,
    setError,
    reload,
    loadMore,
    removeItem,
  };
}
