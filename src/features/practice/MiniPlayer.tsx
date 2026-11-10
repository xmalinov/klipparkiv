import { formatDuration } from "../../api/tasks";
import { useI18n } from "../../i18n";
import { usePractice } from "./PracticeProvider";

export default function MiniPlayer({ onOpen }: { onOpen: () => void }) {
  const { t } = useI18n();
  const { track, state, snapshot, playPause } = usePractice();

  if (!track) return null;
  const active = state.regions.find(
    (region) => region.id === state.activeRegionId,
  );

  return (
    <div className="mini-player" role="region" aria-label={t("practice.title")}>
      <button
        type="button"
        className="transport-play small"
        aria-label={snapshot.playing ? t("practice.pause") : t("practice.play")}
        onClick={playPause}
      >
        {snapshot.playing ? "❚❚" : "▶"}
      </button>
      <button type="button" className="mini-title" onClick={onOpen} title={track.title}>
        {track.title}
      </button>
      <span className="mini-time">
        {formatDuration(snapshot.positionSecs)} / {formatDuration(snapshot.durationSecs ?? 0)}
      </span>
      <span className="mini-meta">{state.speed}×</span>
      {active && (
        <span className="mini-meta" style={{ color: active.color }}>
          {active.name}
        </span>
      )}
      <button type="button" className="btn ghost" onClick={onOpen}>
        {t("practice.mini.open")}
      </button>
    </div>
  );
}
