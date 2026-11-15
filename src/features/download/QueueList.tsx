import { useI18n, type Translate } from "../../i18n";
import { stateLabel, type TaskSnapshot } from "../../api/tasks";
import { formatEta, formatSpeed } from "../../lib/format";

function taskSubtitle(task: TaskSnapshot, t: Translate): string {
  const base =
    task.mode === "audio" ? t("download.task.audio") : t("download.task.video");
  const parts = [task.clip ? t("download.task.clip", { base }) : base];
  if (task.playlistCount !== null) {
    parts.push(
      task.playlistIndex !== null
        ? t("download.queuePlaylist", {
            index: task.playlistIndex,
            count: task.playlistCount || "?",
          })
        : t("download.queuePlaylistUnknown"),
    );
  }
  return parts.join(" · ");
}

/**
 * The queue table. It is presentation only: the tasks and the actions come from
 * `QueueProvider` through `DownloadScreen`, so leaving this screen does not
 * affect a download in progress.
 */
export default function QueueList({
  tasks,
  paused,
  onTogglePause,
  onTaskAction,
}: {
  tasks: TaskSnapshot[];
  paused: boolean;
  onTogglePause: () => void;
  onTaskAction: (task: TaskSnapshot) => void;
}) {
  const { language, t } = useI18n();

  return (
    <section className="queue" aria-label={t("download.queueLabel")}>
      <div className="queue-top">
        <h2>{t("download.queue")}</h2>
        {tasks.length > 0 && <span className="queue-count">{tasks.length}</span>}
        {tasks.length > 0 && (
          <div className="queue-tools">
            <button type="button" className="link-btn" onClick={onTogglePause}>
              {paused ? t("download.resume") : t("download.pause")}
            </button>
          </div>
        )}
      </div>

      {tasks.length === 0 ? (
        <p className="queue-empty">{t("download.queueEmpty")}</p>
      ) : (
        <div className="qtable">
          <div className="qhead">
            <span />
            <span>{t("download.colName")}</span>
            <span>{t("download.colStatus")}</span>
            <span>{t("download.colProgress")}</span>
            <span>{t("download.colSpeed")}</span>
            <span />
          </div>
          {tasks.map((task) => {
            const label = task.title && task.title.trim() ? task.title : task.url;
            const status =
              task.state === "running"
                ? [formatSpeed(task.speedBps, language), formatEta(task.etaSecs)]
                    .filter(Boolean)
                    .join(" · ")
                : task.state === "skipped"
                  ? t("download.alreadyDownloaded")
                  : task.state === "partial"
                    ? (task.error ?? t("download.partialHint"))
                    : (task.error ?? "—");
            return (
              <div className="qrow" key={task.id}>
                <span className={`qdot ${task.state}`} aria-hidden="true" />
                <div className="qname">
                  <b title={label}>{label}</b>
                  <span>{taskSubtitle(task, t)}</span>
                </div>
                <div className="qstatus">{stateLabel(task, language)}</div>
                <div className="qprogress">
                  <div className="qbar">
                    <i style={{ width: `${Math.min(100, task.percent)}%` }} />
                  </div>
                </div>
                <div className="qspeed" title={task.error ?? undefined}>
                  {status}
                </div>
                <button
                  type="button"
                  className="icon-btn"
                  aria-label={t("download.cancelTask", { url: task.url })}
                  title={
                    task.state === "running"
                      ? t("download.cancel")
                      : t("download.removeTask")
                  }
                  onClick={() => onTaskAction(task)}
                >
                  ×
                </button>
              </div>
            );
          })}
        </div>
      )}
    </section>
  );
}
