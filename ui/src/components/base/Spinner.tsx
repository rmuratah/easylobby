type Props = {
  size?: number;
  /** Texto para leitores de tela; `null` quando o carregamento já é anunciado por outro elemento. */
  label?: string | null;
};

export function Spinner({ size = 24, label = "Carregando" }: Props) {
  const svg = (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true" className="motion-safe:animate-spin">
      <circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" strokeOpacity="0.25" strokeWidth="3" />
      <path d="M12 3a9 9 0 0 1 9 9" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" />
    </svg>
  );
  if (label === null) return svg;
  return (
    <span role="status" aria-label={label} className="inline-flex text-accent">
      {svg}
    </span>
  );
}
