import { useEffect, useState } from "react";
import { useI18n } from "../../i18n";
import {
  cancelPendingUpdate,
  checkAppUpdate,
  downloadAppUpdate,
  formatBytes,
  getUpdateStatus,
  installAppUpdate,
  openManualUpdate,
  subscribeUpdate,
  type UpdateStatus,
} from "../../api/updates";

function statusKey(status: UpdateStatus): string {
  return `${status.state}:${status.version ?? status.error ?? ""}`;
}

export default function UpdateBanner() {
  const { language, t } = useI18n();
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  const [dismissed, setDismissed] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    let dispose: (() => void) | undefined;

    getUpdateStatus()
      .then((value) => {
        if (alive && value && typeof value.state === "string") setStatus(value);
      })
      .catch(() => {});

    subscribeUpdate((value) => {
      if (alive) setStatus(value);
    }).then((unlisten) => {
      if (alive) dispose = unlisten;
      else unlisten();
    });

    return () => {
      alive = false;
      dispose?.();
    };
  }, []);

  if (!status) return null;

  const visibleStates = [
    "available",
    "downloading",
    "ready",
    "deferred",
    "installing",
    "manual",
    "error",
  ];
  if (!visibleStates.includes(status.state)) return null;

  const transient = ["downloading", "deferred", "installing"].includes(status.state);
  if (!transient && dismissed === statusKey(status)) return null;

  async function run(action: () => Promise<UpdateStatus>) {
    setBusy(true);
    setActionError(null);
    try {
      const next = await action();
      if (next && typeof next.state === "string") setStatus(next);
    } catch (cause) {
      setActionError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  const error = actionError ?? status.error;
  const percent = Math.round(status.percent);

  return (
    <section className="update-banner" role="status" aria-live="polite">
      <div className="update-banner-text">
        <strong>
          {status.state === "available" && status.version
            ? t("update.available", { version: status.version })
            : status.state === "ready" && status.version
              ? t("update.ready", { version: status.version })
              : status.state === "downloading"
                ? t("update.downloading", { percent })
                : status.state === "deferred"
                  ? t("update.deferred")
                  : status.state === "installing"
                    ? t("update.installing")
                    : status.state === "manual"
                      ? t("update.manual", { version: status.version ?? "" })
                      : t("update.error", { error: error ?? "" })}
        </strong>
        {status.state === "available" && status.size !== null && (
          <span className="update-size">{formatBytes(status.size, language)}</span>
        )}
        {status.state === "available" && status.notes && (
          <span className="update-notes">{status.notes}</span>
        )}
        {status.state === "downloading" && (
          <div
            className="update-progress"
            role="progressbar"
            aria-label={t("settings.appUpdate")}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={percent}
          >
            <i style={{ width: `${Math.min(100, Math.max(0, percent))}%` }} />
          </div>
        )}
      </div>

      <div className="update-banner-actions">
        {status.state === "available" && (
          <button
            type="button"
            className="btn primary"
            disabled={busy}
            onClick={() => run(downloadAppUpdate)}
          >
            {t("update.download")}
          </button>
        )}
        {status.state === "ready" && (
          <button
            type="button"
            className="btn primary"
            disabled={busy}
            onClick={() => run(installAppUpdate)}
          >
            {t("update.restart")}
          </button>
        )}
        {status.state === "manual" && (
          <button
            type="button"
            className="btn primary"
            disabled={busy}
            onClick={() => run(openManualUpdate)}
          >
            {t("update.openDmg")}
          </button>
        )}
        {status.state === "deferred" && (
          <button
            type="button"
            className="btn ghost"
            disabled={busy}
            onClick={() => run(cancelPendingUpdate)}
          >
            {t("update.cancelPending")}
          </button>
        )}
        {status.state === "error" && (
          <button
            type="button"
            className="btn primary"
            disabled={busy}
            onClick={() => run(() => checkAppUpdate(false))}
          >
            {t("update.retry")}
          </button>
        )}
        {!["downloading", "deferred", "installing"].includes(status.state) && (
          <button
            type="button"
            className="btn ghost"
            onClick={() => setDismissed(statusKey(status))}
          >
            {t("update.later")}
          </button>
        )}
      </div>
    </section>
  );
}
