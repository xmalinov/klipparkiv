import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { listen } from "@tauri-apps/api/event";
import {
  cancelImport as cancelImportCommand,
  importMedia,
  type ImportProgress,
  type ImportReport,
} from "../../api/library";
import { useI18n } from "../../i18n";

interface ImportContextValue {
  progress: ImportProgress | null;
  importing: boolean;
  cancelling: boolean;
  error: string | null;
  startImport: (paths: string[]) => Promise<ImportReport>;
  cancelImport: () => void;
  clearError: () => void;
}

const ImportContext = createContext<ImportContextValue | null>(null);

export function ImportProvider({ children }: { children: ReactNode }) {
  const { t } = useI18n();
  const [progress, setProgress] = useState<ImportProgress | null>(null);
  const [importing, setImporting] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const importingRef = useRef(false);

  useEffect(() => {
    let alive = true;
    let unlisten: (() => void) | undefined;
    listen<ImportProgress>("library://import-progress", (event) => {
      setProgress(event.payload);
    })
      .then((stop) => {
        if (alive) unlisten = stop;
        else stop();
      })
      .catch(() => {});
    return () => {
      alive = false;
      unlisten?.();
    };
  }, []);

  const startImport = useCallback(
    async (paths: string[]): Promise<ImportReport> => {
      if (importingRef.current) {
        throw new Error(t("library.importInProgress"));
      }
      importingRef.current = true;
      setImporting(true);
      setCancelling(false);
      setError(null);
      setProgress(null);
      try {
        return await importMedia(paths);
      } catch (cause) {
        setError(String(cause));
        throw cause;
      } finally {
        importingRef.current = false;
        setImporting(false);
        setCancelling(false);
        setProgress(null);
      }
    },
    [t],
  );

  const cancelImport = useCallback(() => {
    if (!importingRef.current) return;
    setCancelling(true);
    void cancelImportCommand().catch(() => {});
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (!importingRef.current || event.key !== "Escape") return;
      // The import owns Esc while it runs; do not also close menus.
      event.preventDefault();
      event.stopImmediatePropagation();
      cancelImport();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [cancelImport]);

  const value = useMemo<ImportContextValue>(
    () => ({
      progress,
      importing,
      cancelling,
      error,
      startImport,
      cancelImport,
      clearError: () => setError(null),
    }),
    [cancelImport, cancelling, error, importing, progress, startImport],
  );

  return (
    <ImportContext.Provider value={value}>{children}</ImportContext.Provider>
  );
}

export function useImport(): ImportContextValue | null {
  return useContext(ImportContext);
}
