import { useEffect, useState } from "react";
import { useI18n, type TranslationKey } from "../../i18n";
import type { RegionKind } from "../../api/practice";
import { REGION_COLORS, REGION_KINDS } from "./PracticeProvider";

const KIND_KEYS: Record<RegionKind, TranslationKey> = {
  intro: "practice.regionKinds.intro",
  verse: "practice.regionKinds.verse",
  chorus: "practice.regionKinds.chorus",
  solo: "practice.regionKinds.solo",
  bridge: "practice.regionKinds.bridge",
  outro: "practice.regionKinds.outro",
  custom: "practice.regionKinds.custom",
};

export interface RegionDraft {
  name: string;
  kind: RegionKind;
  color: string;
}

export default function RegionDialog({
  initial,
  onCancel,
  onSave,
}: {
  initial?: RegionDraft;
  onCancel: () => void;
  onSave: (draft: RegionDraft) => void;
}) {
  const { t } = useI18n();
  const [name, setName] = useState(initial?.name ?? "");
  const [kind, setKind] = useState<RegionKind>(initial?.kind ?? "custom");
  const [color, setColor] = useState(
    initial?.color ?? REGION_COLORS.custom,
  );

  useEffect(() => {
    setName(initial?.name ?? "");
    setKind(initial?.kind ?? "custom");
    setColor(initial?.color ?? REGION_COLORS.custom);
  }, [initial]);

  return (
    <div className="overlay" role="presentation" onClick={onCancel}>
      <div
        className="dialog practice-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={t("practice.addRegion")}
        onClick={(event) => event.stopPropagation()}
      >
        <div className="dialog-head">
          <span>{t("practice.addRegion")}</span>
          <button
            type="button"
            className="icon-btn"
            aria-label={t("common.close")}
            onClick={onCancel}
          >
            ×
          </button>
        </div>
        <div className="dialog-body">
          <div className="row">
            <label className="label" htmlFor="region-name">
              {t("practice.regionName")}
            </label>
            <input
              id="region-name"
              className="field"
              value={name}
              autoFocus
              onChange={(event) => setName(event.currentTarget.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && name.trim()) {
                  onSave({ name: name.trim(), kind, color });
                }
              }}
            />
          </div>
          <div className="row">
            <label className="label" htmlFor="region-kind">
              {t("practice.regions")}
            </label>
            <select
              id="region-kind"
              className="field"
              value={kind}
              onChange={(event) => {
                const next = event.currentTarget.value as RegionKind;
                setKind(next);
                setColor(REGION_COLORS[next]);
              }}
            >
              {REGION_KINDS.map((value) => (
                <option key={value} value={value}>
                  {t(KIND_KEYS[value])}
                </option>
              ))}
            </select>
          </div>
          <div className="row">
            <label className="label" htmlFor="region-color">
              {t("practice.regions")}
            </label>
            <input
              id="region-color"
              type="color"
              className="practice-color"
              value={color}
              onChange={(event) => setColor(event.currentTarget.value)}
            />
          </div>
        </div>
        <div className="dialog-foot">
          <button type="button" className="btn ghost" onClick={onCancel}>
            {t("common.cancel")}
          </button>
          <button
            type="button"
            className="btn primary"
            disabled={!name.trim()}
            onClick={() => onSave({ name: name.trim(), kind, color })}
          >
            {t("common.save")}
          </button>
        </div>
      </div>
    </div>
  );
}
