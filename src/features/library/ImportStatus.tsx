import { useI18n } from "../../i18n";
import { useImport } from "./ImportProvider";

export default function ImportStatus() {
  const { t } = useI18n();
  const importer = useImport();
  if (!importer?.importing) return null;

  const label = importer.progress
    ? t("library.importing", {
        index: importer.progress.index,
        total: importer.progress.total,
        name: importer.progress.name,
      })
    : t("library.importStarting");

  return (
    <div className="import-chip" role="status" aria-live="polite">
      <span className="import-spinner" aria-hidden="true" />
      <span className="import-chip-text" title={label}>
        {label}
      </span>
      <button
        type="button"
        className="btn ghost"
        onClick={importer.cancelImport}
        disabled={importer.cancelling}
      >
        {importer.cancelling
          ? t("library.cancelling")
          : t("library.cancelImport")}
      </button>
    </div>
  );
}
