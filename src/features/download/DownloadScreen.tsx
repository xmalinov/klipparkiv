import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getSettings } from "../../api/settings";
import {
  applyProgress,
  cancelTask,
  enqueueBatch,
  enqueueTask,
  formatDuration,
  formatEta,
  formatSpeed,
  formatUploadDate,
  formatViews,
  pauseQueue,
  probeMetadata,
  resumeQueue,
  stateLabel,
  subscribeTasks,
  upsertTask,
  type DownloadMode,
  type QueueState,
  type TaskSnapshot,
  type VideoInfo,
} from "../../api/tasks";

const MODES: { id: DownloadMode; label: string }[] = [
  { id: "audio", label: "Аудио" },
  { id: "video", label: "Видео" },
  { id: "both", label: "Аудио+Видео" },
  { id: "clip", label: "Клип" },
];

function modeHint(
  mode: DownloadMode,
  quality: string,
  clipKind: "audio" | "video",
  audioKbps: number,
): string {
  const videoQuality = quality === "best" ? "лучшее качество" : `${quality}p`;
  switch (mode) {
    case "audio":
      return `MP3 ${audioKbps} kbps · обложка и метаданные внутри файла`;
    case "video":
      return `MP4 ${videoQuality} · слияние в один файл`;
    case "both":
      return `MP3 ${audioKbps} kbps + MP4 ${videoQuality} · две задачи, два архива`;
    case "clip":
      return clipKind === "audio"
        ? `Фрагмент · MP3 ${audioKbps} kbps`
        : `Фрагмент · MP4 ${videoQuality}`;
  }
}

function taskLabel(task: TaskSnapshot): string {
  const base = task.mode === "audio" ? "Аудио" : "Видео";
  return task.clip ? `${base} · клип` : base;
}

export default function DownloadScreen({ root }: { root?: string }) {
  const [url, setUrl] = useState("");
  const [mode, setMode] = useState<DownloadMode>("audio");
  const [clipKind, setClipKind] = useState<"audio" | "video">("video");
  const [quality, setQuality] = useState("1080");
  const [audioKbps, setAudioKbps] = useState(320);
  const [outDir, setOutDir] = useState<string | null>(null);
  const [audioSubdir, setAudioSubdir] = useState("audio");
  const [videoSubdir, setVideoSubdir] = useState("video");
  const [clipStart, setClipStart] = useState("");
  const [clipEnd, setClipEnd] = useState("");
  const [meta, setMeta] = useState<VideoInfo | null>(null);
  const [probing, setProbing] = useState(false);
  const [metaError, setMetaError] = useState<string | null>(null);
  const [tasks, setTasks] = useState<TaskSnapshot[]>([]);
  const [queue, setQueue] = useState<QueueState>({
    paused: false,
    running: 0,
    queued: 0,
  });
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let alive = true;
    getSettings()
      .then((settings) => {
        if (!alive) return;
        setAudioSubdir(settings.audioSubdir || "audio");
        setVideoSubdir(settings.videoSubdir || "video");
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    let alive = true;
    let dispose: (() => void) | undefined;

    subscribeTasks({
      onState: (task) => setTasks((current) => upsertTask(current, task)),
      onProgress: (progress) =>
        setTasks((current) => applyProgress(current, progress)),
      onQueue: (state) => setQueue(state),
    }).then((unlisten) => {
      if (alive) dispose = unlisten;
      else unlisten();
    });

    return () => {
      alive = false;
      dispose?.();
    };
  }, []);

  useEffect(() => {
    const trimmed = url.trim();
    setMeta(null);
    setMetaError(null);
    if (!/^https?:\/\//.test(trimmed)) {
      setProbing(false);
      return;
    }

    let alive = true;
    const timer = setTimeout(() => {
      setProbing(true);
      probeMetadata(trimmed)
        .then((info) => {
          if (alive) setMeta(info);
        })
        .catch((cause: unknown) => {
          if (alive) setMetaError(String(cause));
        })
        .finally(() => {
          if (alive) setProbing(false);
        });
    }, 700);

    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [url]);

  const clipReady =
    mode !== "clip" ||
    (clipStart.trim().length > 0 && clipEnd.trim().length > 0);
  const isClip = mode === "clip";
  const effectiveMode = isClip ? clipKind : mode;
  const wantsVideo = effectiveMode === "video" || effectiveMode === "both";
  const wantsAudio = effectiveMode === "audio" || effectiveMode === "both";
  const defaultSubdir = wantsVideo && !wantsAudio ? videoSubdir : audioSubdir;
  const targetDir =
    outDir ??
    (effectiveMode === "both"
      ? `${root ?? ""} · ${audioSubdir}\\ и ${videoSubdir}\\`
      : `${root ?? ""}\\${defaultSubdir}`);
  const outsideRoot =
    outDir !== null && root !== undefined && !outDir.toLowerCase().startsWith(root.toLowerCase());
  const canSubmit = url.trim().length > 0 && clipReady && !busy;

  async function pickDestination() {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected === "string") setOutDir(selected);
    } catch (cause) {
      setError(String(cause));
    }
  }

  async function submit() {
    if (!canSubmit) return;
    setBusy(true);
    setError(null);
    try {
      const created = await enqueueTask({
        url: url.trim(),
        mode: isClip ? clipKind : mode,
        quality: wantsVideo ? quality : undefined,
        audioKbps: wantsAudio ? audioKbps : undefined,
        outDir: outDir ?? undefined,
        clipStart: isClip ? clipStart.trim() : undefined,
        clipEnd: isClip ? clipEnd.trim() : undefined,
      });
      setTasks((current) => created.reduce(upsertTask, current));
      setUrl("");
      setClipStart("");
      setClipEnd("");
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function paste() {
    try {
      const text = await navigator.clipboard.readText();
      if (text.trim()) setUrl(text.trim());
    } catch {
      setError("Буфер обмена недоступен — вставьте ссылку через Ctrl+V");
    }
  }

  async function togglePause() {
    try {
      const next = queue.paused ? await resumeQueue() : await pauseQueue();
      setQueue(next);
    } catch (cause) {
      setError(String(cause));
    }
  }

  async function pickBatch() {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "Списки ссылок", extensions: ["txt"] }],
      });
      if (!selected || Array.isArray(selected)) return;
      const created = await enqueueBatch({
        path: selected,
        mode: isClip ? clipKind : mode,
        quality: wantsVideo ? quality : undefined,
      });
      setTasks((current) => created.reduce(upsertTask, current));
    } catch (cause) {
      setError(String(cause));
    }
  }

  return (
    <div className="download">
      <div className="url-row">
        <input
          className={error ? "field input invalid" : "field input"}
          value={url}
          onChange={(event) => {
            setUrl(event.currentTarget.value);
            setError(null);
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter") submit();
          }}
          placeholder="Вставьте ссылку на YouTube или введите её вручную"
          aria-label="Ссылка на видео"
          spellCheck={false}
          autoFocus
        />
        <button type="button" className="btn btn-ghost" onClick={paste}>
          Вставить
        </button>
      </div>

      {error && (
        <p className="field-error" role="alert">
          {error}
        </p>
      )}

      <div className="modes">
        <div className="tabs" role="tablist" aria-label="Режим загрузки">
          {MODES.map((item) => (
            <button
              key={item.id}
              type="button"
              role="tab"
              aria-selected={mode === item.id}
              className={mode === item.id ? "tab active" : "tab"}
              onClick={() => setMode(item.id)}
            >
              {item.label}
            </button>
          ))}
        </div>

        {(wantsVideo || wantsAudio) && (
          <div className="quality-group">
            {wantsAudio && (
              <label className="quality">
                <span>Аудио</span>
                <select
                  aria-label="Качество аудио"
                  value={audioKbps}
                  onChange={(event) => setAudioKbps(Number(event.currentTarget.value))}
                >
                  <option value={320}>320 kbps</option>
                  <option value={256}>256 kbps</option>
                  <option value={192}>192 kbps</option>
                  <option value={128}>128 kbps</option>
                </select>
              </label>
            )}
            {wantsVideo && (
              <label className="quality">
                <span>Видео</span>
                <select
                  aria-label="Качество видео"
                  value={quality}
                  onChange={(event) => setQuality(event.currentTarget.value)}
                >
                  <option value="2160">2160p</option>
                  <option value="1440">1440p</option>
                  <option value="1080">1080p</option>
                  <option value="720">720p</option>
                  <option value="480">480p</option>
                  <option value="best">Лучшее</option>
                </select>
              </label>
            )}
          </div>
        )}
      </div>

      <div className="hint-row">
        <p className="hint">{modeHint(mode, quality, clipKind, audioKbps)}</p>
        <button type="button" className="link-btn" onClick={pickBatch}>
          Из файла со ссылками…
        </button>
      </div>

      {mode === "clip" && (
        <div className="clip-row">
          <div className="clip-kind" role="group" aria-label="Тип клипа">
            <button
              type="button"
              className={clipKind === "audio" ? "chip active" : "chip"}
              onClick={() => setClipKind("audio")}
            >
              Аудио
            </button>
            <button
              type="button"
              className={clipKind === "video" ? "chip active" : "chip"}
              onClick={() => setClipKind("video")}
            >
              Видео
            </button>
          </div>
          <label>
            Начало
            <input
              className="field clip"
              value={clipStart}
              onChange={(event) => setClipStart(event.currentTarget.value)}
              placeholder="00:01:30"
              inputMode="numeric"
              aria-label="Начало клипа"
            />
          </label>
          <span className="clip-sep">—</span>
          <label>
            Конец
            <input
              className="field clip"
              value={clipEnd}
              onChange={(event) => setClipEnd(event.currentTarget.value)}
              placeholder="00:05:00"
              inputMode="numeric"
              aria-label="Конец клипа"
            />
          </label>
        </div>
      )}

      {probing && <p className="meta-loading">Проверяем ссылку…</p>}

      {meta && !probing && (
        <div className="meta">
          {meta.thumbnailUrl ? (
            <img
              className="thumb"
              src={meta.thumbnailUrl}
              alt=""
              loading="lazy"
            />
          ) : (
            <div className="thumb thumb-empty" aria-hidden="true" />
          )}
          <div className="meta-body">
            <div className="meta-title" title={meta.title}>
              {meta.title}
            </div>
            <div className="meta-sub">
              {[
                meta.channel,
                formatViews(meta.viewCount),
                formatUploadDate(meta.uploadDate),
              ]
                .filter(Boolean)
                .join(" · ")}
            </div>
            <div className="meta-formats">
              {[formatDuration(meta.durationSecs), "обложка в кэше"]
                .filter(Boolean)
                .join(" · ")}
            </div>
          </div>
        </div>
      )}

      {metaError && !probing && (
        <p className="meta-error" role="status">
          {metaError}
        </p>
      )}

      <div className="dest">
        <span className="dest-line">
          Папка: <span className="mono" title={targetDir}>{targetDir}</span>
        </span>
        <button type="button" className="link-btn" onClick={pickDestination}>
          Изменить
        </button>
        {outDir && (
          <button type="button" className="link-btn" onClick={() => setOutDir(null)}>
            По умолчанию
          </button>
        )}
      </div>
      {outsideRoot && (
        <p className="hint warning">
          Папка вне приложения — файл не попадёт в библиотеку
        </p>
      )}

      <div className="actions">
        <button
          type="button"
          className="btn primary"
          onClick={submit}
          disabled={!canSubmit}
        >
          Скачать
        </button>
      </div>

      <section className="queue" aria-label="Очередь загрузок">
        <div className="queue-top">
          <h2>Очередь</h2>
          {tasks.length > 0 && <span className="queue-count">{tasks.length}</span>}
          {tasks.length > 0 && (
            <div className="queue-tools">
              <button type="button" className="link-btn" onClick={togglePause}>
                {queue.paused ? "Продолжить" : "Пауза"}
              </button>
            </div>
          )}
        </div>

        {tasks.length === 0 ? (
          <p className="queue-empty">Здесь появятся загрузки</p>
        ) : (
          <div className="qtable">
            <div className="qhead">
              <span />
              <span>Название</span>
              <span>Статус</span>
              <span>Прогресс</span>
              <span>Скорость</span>
              <span />
            </div>
            {tasks.map((task) => (
              <div className="qrow" key={task.id}>
                <span className={`qdot ${task.state}`} aria-hidden="true" />
                <div className="qname">
                  <b title={task.url}>{task.url}</b>
                  <span>{taskLabel(task)}</span>
                </div>
                <div className="qstatus">{stateLabel(task)}</div>
                <div className="qprogress">
                  <div className="qbar">
                    <i style={{ width: `${Math.min(100, task.percent)}%` }} />
                  </div>
                </div>
                <div className="qspeed" title={task.error ?? undefined}>
                  {task.state === "running"
                    ? [formatSpeed(task.speedBps), formatEta(task.etaSecs)]
                        .filter(Boolean)
                        .join(" · ")
                    : (task.error ?? "—")}
                </div>
                <button
                  type="button"
                  className="icon-btn"
                  aria-label={`Отменить ${task.url}`}
                  title={task.state === "running" ? "Отменить" : "Убрать"}
                  onClick={() => cancelTask(task.id)}
                >
                  ×
                </button>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
