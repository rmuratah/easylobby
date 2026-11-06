import { useState } from "react";
import { cx } from "./cx";
import { initials } from "./initials";

type Props = {
  /** Nome do invocador ou campeão: vira as iniciais quando não há imagem. */
  name: string;
  src?: string;
  size?: number;
  shape?: "circle" | "rounded";
  /** Mostra o nome para leitores de tela; deixe falso quando o nome já aparece ao lado. */
  labelled?: boolean;
  className?: string;
};

export function Avatar({ name, src, size = 36, shape = "circle", labelled = false, className }: Props) {
  const [failedSrc, setFailedSrc] = useState<string | null>(null);
  const showImage = src && failedSrc !== src;
  const style = { width: size, height: size, fontSize: Math.round(size * 0.36) };
  const classes = cx(
    "inline-flex shrink-0 items-center justify-center overflow-hidden bg-raised font-semibold text-ink",
    shape === "circle" ? "rounded-full" : "rounded-control",
    className,
  );
  const a11y = labelled ? { role: "img", "aria-label": name } : { "aria-hidden": true };
  return (
    <span className={classes} style={style} {...a11y}>
      {showImage ? (
        <img src={src} alt="" className="h-full w-full object-cover" onError={() => setFailedSrc(src)} />
      ) : (
        initials(name)
      )}
    </span>
  );
}
