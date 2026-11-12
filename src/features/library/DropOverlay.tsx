export default function DropOverlay({
  title,
  hint,
}: {
  title: string;
  hint?: string;
}) {
  return (
    <div className="drop-overlay global" role="status" aria-live="polite">
      <div className="drop-card">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M12 16V4" />
          <path d="m7 9 5-5 5 5" />
          <path d="M4 16v3a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1v-3" />
        </svg>
        <strong>{title}</strong>
        {hint && <span>{hint}</span>}
      </div>
    </div>
  );
}
