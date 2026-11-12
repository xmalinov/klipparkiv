import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { readTextFile } from "../../api/library";
import { getSettings } from "../../api/settings";
import { useI18n, type Translate } from "../../i18n";
import DropOverlay from "../library/DropOverlay";
import { useFileDrop } from "../library/useFileDrop";
import {
  applyProgress,
  cancelTask,
  enqueueBatch,
  enqueueLinks,
  enqueueTask,
  formatDuration,
  formatEta,
  formatSpeed,
  formatTimecode,
  formatUploadDate,
  formatViews,
  parseTimecode,
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

const MODES: {
  id: DownloadMode;
  labelKey:
    | "download.mode.audio"
    | "download.mode.video"
    | "download.mode.both"
    | "download.mode.clip";
}[] = [
  { id: "audio", labelKey: "download.mode.audio" },
  { id: "video", labelKey: "download.mode.video" },
  { id: "both", labelKey: "download.mode.both" },
  { id: "clip", labelKey: "download.mode.clip" },
];

/** Matches `MAX_PLAYLIST_ITEMS` in the backend. */
const MAX_PLAYLIST_ITEMS = 500;

type PlaylistChoice = "video" | "all";

function modeHint(
  mode: DownloadMode,
  quality: string,
  clipKind: "audio" | "video",
  audioKbps: number,
  t: Translate,
): string {
  const videoQuality =
    quality === "best" ? t("download.bestQuality") : `${quality}p`;
  switch (mode) {
    case "audio":
      return t("download.hint.audio", { kbps: audioKbps });
    case "video":
      return t("download.hint.video", { quality: videoQuality });
    case "both":
      return t("download.hint.both", {
        kbps: audioKbps,
        quality: videoQuality,
      });
    case "clip":
      return clipKind === "audio"
        ? t("download.hint.clipAudio", { kbps: audioKbps })
        : t("download.hint.clipVideo", { quality: videoQuality });
  }
}

/**
 * `watch?...&list=...` usually means "this video", while a bare
 * `/playlist?list=...` link means the whole list.
 */
function defaultPlaylistChoice(url: string): PlaylistChoice {
  try {
    return new URL(url).pathname.startsWith("/playlist") ? "all" : "video";
  } catch {
    return "video";
  }
}

function firstLink(text: string): string {
  return (
    text
      .split(/\r?\n/)
      .map((line) => line.trim())
      .find((line) => /^https?:\/\//.test(line)) ?? ""
  );
}

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

export default function DownloadScreen({ root }: { root?: string }) {
  const { language, t } = useI18n();
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
  const [playlistChoice, setPlaylistChoice] =
    useState<PlaylistChoice>("video");
  const [playlistStart, setPlaylistStart] = useState("");
  const [playlistEnd, setPlaylistEnd] = useState("");
  const [listOpen, setListOpen] = useState(false);
  const [listText, setListText] = useState("");
  const [tasks, setTasks] = useState<TaskSnapshot[]>([]);
  const [queue, setQueue] = useState<QueueState>({
    paused: false,
    running: 0,
    queued: 0,
  });
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const drop = useFileDrop({ onDrop: handlePaths });

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
    setPlaylistChoice(defaultPlaylistChoice(trimmed));
    setPlaylistStart("");
    setPlaylistEnd("");
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

  const isClip = mode === "clip";
  const effectiveMode: DownloadMode = isClip ? clipKind : mode;
  const wantsVideo = effectiveMode === "video" || effectiveMode === "both";
  const wantsAudio = effectiveMode === "audio" || effectiveMode === "both";
  const pathSeparator = navigator.userAgent.includes("Mac") ? "/" : "\\";
  const defaultSubdir = wantsVideo && !wantsAudio ? videoSubdir : audioSubdir;
  const targetDir =
    outDir ??
    (effectiveMode === "both"
      ? `${root ?? ""} · ${audioSubdir}${pathSeparator} ${language === "ru" ? "и" : "and"} ${videoSubdir}${pathSeparator}`
      : `${root ?? ""}\\${defaultSubdir}`);
  const outsideRoot =
    outDir !== null && root !== undefined && !outDir.toLowerCase().startsWith(root.toLowerCase());

  const clipStartSecs = parseTimecode(clipStart);
  const clipEndSecs = parseTimecode(clipEnd);
  const clipStartInvalid =
    isClip && clipStart.trim() !== "" && clipStartSecs === null;
  const clipEndInvalid = isClip && clipEnd.trim() !== "" && clipEndSecs === null;
  const clipOrderInvalid =
    isClip &&
    clipStartSecs !== null &&
    clipEndSecs !== null &&
    clipStartSecs >= clipEndSecs;
  const clipError =
    clipStartInvalid || clipEndInvalid
      ? t("download.clipInvalid")
      : clipOrderInvalid
        ? t("download.clipOrder")
        : null;

  const playlistInfo = meta?.kind === "playlist" ? meta : null;
  const playlistTotal = playlistInfo?.playlistCount ?? 0;
  // A bare `/playlist?list=` link is a playlist even before the probe returns,
  // so a quick Enter cannot silently download only the first video.
  const urlLooksLikePlaylist = defaultPlaylistChoice(url.trim()) === "all";
  const showPlaylistChoice = playlistInfo !== null || urlLooksLikePlaylist;
  const playlistAll = playlistChoice === "all" && !isClip;

  const rangeStart = playlistStart.trim() ? Number(playlistStart) : null;
  const rangeEnd = playlistEnd.trim() ? Number(playlistEnd) : null;
  const rangeInvalid =
    (rangeStart !== null &&
      (!Number.isInteger(rangeStart) ||
        rangeStart < 1 ||
        rangeStart > MAX_PLAYLIST_ITEMS)) ||
    (rangeEnd !== null &&
      (!Number.isInteger(rangeEnd) ||
        rangeEnd < 1 ||
        rangeEnd > MAX_PLAYLIST_ITEMS)) ||
    (rangeStart !== null && rangeEnd !== null && rangeStart > rangeEnd);
  const playlistError = playlistAll && rangeInvalid;

  const duration =
    meta?.kind === "video" && meta.durationSecs !== null
      ? Math.floor(meta.durationSecs)
      : null;
  const clipBeyondEnd =
    isClip &&
    duration !== null &&
    clipEndSecs !== null &&
    clipEndSecs > duration;

  let clipHint: string | null = null;
  if (isClip && duration !== null) {
    const start = clipStart.trim() ? clipStartSecs : 0;
    const end = clipEnd.trim() ? clipEndSecs : duration;
    if (start !== null && end !== null && end > start) {
      const length = formatDuration(end - start);
      clipHint = clipEnd.trim()
        ? t("download.clipLength", { length })
        : t("download.clipToEndHint", { length });
    }
  }

  const canSubmit =
    url.trim().length > 0 && !busy && !clipError && !playlistError;

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
        mode: effectiveMode,
        quality: wantsVideo ? quality : undefined,
        audioKbps: wantsAudio ? audioKbps : undefined,
        outDir: outDir ?? undefined,
        clipStart: isClip && clipStart.trim() ? clipStart.trim() : undefined,
        clipEnd: isClip && clipEnd.trim() ? clipEnd.trim() : undefined,
        title:
          meta && (meta.kind === "video" || playlistAll)
            ? meta.title
            : undefined,
        playlist: playlistAll ? true : undefined,
        playlistStart:
          playlistAll && rangeStart !== null ? rangeStart : undefined,
        playlistEnd: playlistAll && rangeEnd !== null ? rangeEnd : undefined,
      });
      setTasks((current) => created.reduce(upsertTask, current));
      setUrl("");
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function submitList(text: string) {
    if (!text.trim()) {
      setError(t("download.linksEmpty"));
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const created = await enqueueLinks({
        text,
        mode: effectiveMode,
        quality: wantsVideo ? quality : undefined,
        audioKbps: wantsAudio ? audioKbps : undefined,
        outDir: outDir ?? undefined,
      });
      setTasks((current) => created.reduce(upsertTask, current));
      setListText("");
      setListOpen(false);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function handlePaths(paths: string[]) {
    const lists = paths.filter((path) => path.toLowerCase().endsWith(".txt"));
    if (lists.length === 0) {
      setError(t("download.dropUnsupported"));
      return;
    }
    try {
      const parts = await Promise.all(lists.map((path) => readTextFile(path)));
      const text = parts.join("\n").trim();
      if (!text) {
        setError(t("download.linksEmpty"));
        return;
      }
      await submitList(text);
    } catch (cause) {
      setError(String(cause));
    }
  }

  async function paste() {
    try {
      const text = await navigator.clipboard.readText();
      if (text.trim()) setUrl(text.trim());
    } catch {
      setError(t("download.clipboardError"));
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
        filters: [{ name: t("download.batchFilter"), extensions: ["txt"] }],
      });
      if (!selected || Array.isArray(selected)) return;
      const created = await enqueueBatch({
        path: selected,
        mode: effectiveMode,
        quality: wantsVideo ? quality : undefined,
        audioKbps: wantsAudio ? audioKbps : undefined,
        outDir: outDir ?? undefined,
      });
      setTasks((current) => created.reduce(upsertTask, current));
    } catch (cause) {
      setError(String(cause));
    }
  }

  const sliderMax = duration !== null ? Math.max(0, duration) : 0;
  const sliderStart =
    duration !== null
      ? Math.min(duration, clipStart.trim() ? (clipStartSecs ?? 0) : 0)
      : 0;
  const sliderEnd =
    duration !== null
      ? Math.min(
          duration,
          clipEnd.trim() ? (clipEndSecs ?? duration) : duration,
        )
      : 0;

  return (
    <div className="download">
      {drop.dragging && (
        <DropOverlay
          title={t("download.dropActive")}
          hint={t("download.dropHint")}
        />
      )}

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
          placeholder={t("download.placeholder")}
          aria-label={t("download.urlLabel")}
          spellCheck={false}
          autoFocus
        />
        <button type="button" className="btn btn-ghost" onClick={paste}>
          {t("download.paste")}
        </button>
      </div>

      {error && (
        <p className="field-error" role="alert">
          {error}
        </p>
      )}

      <div className="modes">
        <div className="tabs" role="tablist" aria-label={t("download.modeGroup")}>
          {MODES.map((item) => (
            <button
              key={item.id}
              type="button"
              role="tab"
              aria-selected={mode === item.id}
              className={mode === item.id ? "tab active" : "tab"}
              onClick={() => setMode(item.id)}
            >
              {t(item.labelKey)}
            </button>
          ))}
        </div>

        {(wantsVideo || wantsAudio) && (
          <div className="quality-group">
            {wantsAudio && (
              <label className="quality">
                <span>{t("common.audio")}</span>
                <select
                  aria-label={t("download.audioQuality")}
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
                <span>{t("common.video")}</span>
                <select
                  aria-label={t("download.videoQuality")}
                  value={quality}
                  onChange={(event) => setQuality(event.currentTarget.value)}
                >
                  <option value="2160">2160p</option>
                  <option value="1440">1440p</option>
                  <option value="1080">1080p</option>
                  <option value="720">720p</option>
                  <option value="480">480p</option>
                  <option value="best">{t("download.quality.best")}</option>
                </select>
              </label>
            )}
          </div>
        )}
      </div>

      <div className="hint-row">
        <p className="hint">
          {modeHint(mode, quality, clipKind, audioKbps, t)}
        </p>
        <span className="hint-actions">
          <button
            type="button"
            className="link-btn"
            onClick={() => setListOpen((open) => !open)}
          >
            {t("download.linksPanel")}
          </button>
          <button type="button" className="link-btn" onClick={pickBatch}>
            {t("download.batch")}
          </button>
        </span>
      </div>

      {listOpen && (
        <div className="list-panel">
          <textarea
            className="field list-text"
            value={listText}
            onChange={(event) => setListText(event.currentTarget.value)}
            placeholder={t("download.linksPlaceholder")}
            aria-label={t("download.linksPanel")}
            rows={4}
            spellCheck={false}
          />
          <div className="list-actions">
            <button
              type="button"
              className="btn primary"
              disabled={!listText.trim() || busy}
              onClick={() => submitList(listText)}
            >
              {t("download.linksAddAll")}
            </button>
            <button
              type="button"
              className="btn btn-ghost"
              disabled={!firstLink(listText)}
              onClick={() => {
                const link = firstLink(listText);
                if (link) {
                  setUrl(link);
                  setListOpen(false);
                }
              }}
            >
              {t("download.linksFirst")}
            </button>
            <span className="hint">{t("download.dropHint")}</span>
          </div>
        </div>
      )}

      {mode === "clip" && (
        <div className="clip-block">
          <div className="clip-row">
            <div className="clip-kind" role="group" aria-label={t("download.clipKind")}>
              <button
                type="button"
                className={effectiveMode === "audio" ? "chip active" : "chip"}
                onClick={() => setClipKind("audio")}
              >
                {t("common.audio")}
              </button>
              <button
                type="button"
                className={effectiveMode === "video" ? "chip active" : "chip"}
                onClick={() => setClipKind("video")}
              >
                {t("common.video")}
              </button>
            </div>
            <label>
              {t("download.clipStart")}
              <input
                className={
                  clipStartInvalid ? "field clip invalid" : "field clip"
                }
                value={clipStart}
                onChange={(event) => setClipStart(event.currentTarget.value)}
                placeholder="00:01:30"
                inputMode="numeric"
                aria-label={t("download.clipStartLabel")}
              />
            </label>
            <span className="clip-sep">—</span>
            <label>
              {t("download.clipEnd")}
              <input
                className={clipEndInvalid ? "field clip invalid" : "field clip"}
                value={clipEnd}
                onChange={(event) => setClipEnd(event.currentTarget.value)}
                placeholder="00:05:00"
                inputMode="numeric"
                aria-label={t("download.clipEndLabel")}
              />
            </label>
            <div className="clip-kind" role="group" aria-label={t("download.clipRange")}>
              <button
                type="button"
                className={!clipStart.trim() ? "chip active" : "chip"}
                onClick={() => setClipStart("")}
              >
                {t("download.clipFromStart")}
              </button>
              <button
                type="button"
                className={!clipEnd.trim() ? "chip active" : "chip"}
                onClick={() => setClipEnd("")}
              >
                {t("download.clipToEnd")}
              </button>
              <button
                type="button"
                className={
                  !clipStart.trim() && !clipEnd.trim() ? "chip active" : "chip"
                }
                onClick={() => {
                  setClipStart("");
                  setClipEnd("");
                }}
              >
                {t("download.clipWhole")}
              </button>
            </div>
            {(clipStart.trim() || clipEnd.trim()) && (
              <button
                type="button"
                className="link-btn"
                onClick={() => {
                  setClipStart("");
                  setClipEnd("");
                }}
              >
                {t("download.clipReset")}
              </button>
            )}
          </div>

          {duration !== null && (
            <div
              className="slider-row"
              role="group"
              aria-label={t("download.clipSlider")}
            >
              <span className="mono">{formatTimecode(sliderStart)}</span>
              <input
                type="range"
                min={0}
                max={Math.max(0, sliderMax - 1)}
                step={1}
                value={sliderStart}
                aria-label={t("download.clipStartSlider")}
                onChange={(event) => {
                  const value = Number(event.currentTarget.value);
                  setClipStart(formatTimecode(value));
                  const currentEnd = clipEnd.trim()
                    ? (clipEndSecs ?? duration)
                    : duration;
                  if (currentEnd <= value && value < duration) {
                    setClipEnd(formatTimecode(value + 1));
                  }
                }}
              />
              <input
                type="range"
                min={1}
                max={sliderMax}
                step={1}
                value={sliderEnd}
                aria-label={t("download.clipEndSlider")}
                onChange={(event) => {
                  const value = Number(event.currentTarget.value);
                  setClipEnd(formatTimecode(value));
                  const currentStart = clipStart.trim()
                    ? (clipStartSecs ?? 0)
                    : 0;
                  if (currentStart >= value) {
                    setClipStart(formatTimecode(Math.max(0, value - 1)));
                  }
                }}
              />
              <span className="mono">{formatTimecode(sliderEnd)}</span>
            </div>
          )}

          {clipError && (
            <p className="field-error" role="alert">
              {clipError}
            </p>
          )}
          {clipBeyondEnd && duration !== null && (
            <p className="hint warning">
              {t("download.clipBeyondEnd", {
                duration: formatDuration(duration),
              })}
            </p>
          )}
          {clipHint && <p className="hint">{clipHint}</p>}
        </div>
      )}

      {probing && <p className="meta-loading">{t("download.probing")}</p>}

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
                meta.kind === "playlist" ? t("download.playlist") : meta.channel,
                meta.kind === "playlist"
                  ? t("download.playlistCount", { count: playlistTotal })
                  : formatViews(meta.viewCount, language),
                meta.kind === "playlist"
                  ? null
                  : formatUploadDate(meta.uploadDate, language),
              ]
                .filter(Boolean)
                .join(" · ")}
            </div>
            {meta.kind === "playlist" ? (
              meta.entries.length > 0 && (
                <ol className="playlist-preview">
                  {meta.entries.map((entry) => (
                    <li key={entry.id} title={entry.title}>
                      {entry.title}
                    </li>
                  ))}
                </ol>
              )
            ) : (
              <div className="meta-formats">
                {[formatDuration(meta.durationSecs), t("download.coverCached")]
                  .filter(Boolean)
                  .join(" · ")}
              </div>
            )}
          </div>
        </div>
      )}

      {metaError && !probing && (
        <p className="meta-error" role="status">
          {metaError}
        </p>
      )}

      {showPlaylistChoice && !probing && (
        <div className="playlist-card">
          <div className="choice" role="group" aria-label={t("download.playlist")}>
            <button
              type="button"
              className={!playlistAll ? "chip active" : "chip"}
              onClick={() => setPlaylistChoice("video")}
            >
              {t("download.playlistSingle")}
            </button>
            <button
              type="button"
              className={playlistAll ? "chip active" : "chip"}
              disabled={isClip}
              title={isClip ? t("download.playlistNoClip") : undefined}
              onClick={() => setPlaylistChoice("all")}
            >
              {t("download.playlistAll")}
            </button>
          </div>
          {playlistAll && (
            <>
              <div className="playlist-range">
                <label>
                  {t("download.playlistRange")}
                  <input
                    className="field clip"
                    type="number"
                    min={1}
                    max={MAX_PLAYLIST_ITEMS}
                    value={playlistStart}
                    onChange={(event) =>
                      setPlaylistStart(event.currentTarget.value)
                    }
                    aria-label={t("download.playlistRange")}
                  />
                </label>
                <label>
                  {t("download.playlistRangeTo")}
                  <input
                    className="field clip"
                    type="number"
                    min={1}
                    max={MAX_PLAYLIST_ITEMS}
                    value={playlistEnd}
                    onChange={(event) =>
                      setPlaylistEnd(event.currentTarget.value)
                    }
                    aria-label={t("download.playlistRangeTo")}
                  />
                </label>
                <span className="hint">{t("download.playlistFolder")}</span>
              </div>
              {playlistTotal > MAX_PLAYLIST_ITEMS && (
                <p className="hint">
                  {t("download.playlistLimit", { count: MAX_PLAYLIST_ITEMS })}
                </p>
              )}
              {playlistTotal > 50 && (
                <p className="hint warning">
                  {t("download.playlistWarning", { count: playlistTotal })}
                </p>
              )}
              {playlistError && (
                <p className="field-error" role="alert">
                  {t("download.playlistInvalid", {
                    count: MAX_PLAYLIST_ITEMS,
                  })}
                </p>
              )}
            </>
          )}
        </div>
      )}

      <div className="dest">
        <span className="dest-line">
          {t("download.folder")}{" "}
          <span className="mono" title={targetDir}>{targetDir}</span>
        </span>
        <button type="button" className="link-btn" onClick={pickDestination}>
          {t("common.change")}
        </button>
        {outDir && (
          <button type="button" className="link-btn" onClick={() => setOutDir(null)}>
            {t("common.default")}
          </button>
        )}
      </div>
      {outsideRoot && (
        <p className="hint warning">{t("download.outsideRoot")}</p>
      )}

      <div className="actions">
        <button
          type="button"
          className="btn primary"
          onClick={submit}
          disabled={!canSubmit}
        >
          {t("download.submit")}
        </button>
      </div>

      <section className="queue" aria-label={t("download.queueLabel")}>
        <div className="queue-top">
          <h2>{t("download.queue")}</h2>
          {tasks.length > 0 && <span className="queue-count">{tasks.length}</span>}
          {tasks.length > 0 && (
            <div className="queue-tools">
              <button type="button" className="link-btn" onClick={togglePause}>
                {queue.paused ? t("download.resume") : t("download.pause")}
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
              const label =
                task.title && task.title.trim() ? task.title : task.url;
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
                    onClick={() => cancelTask(task.id)}
                  >
                    ×
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </section>
    </div>
  );
}
