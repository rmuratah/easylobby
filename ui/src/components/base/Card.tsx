import type { HTMLAttributes } from "react";
import { cx } from "./cx";

type Props = HTMLAttributes<HTMLElement> & { as?: "div" | "section" | "article" | "aside" };

/** Superfície com borda; sem sombra (sombra só em sobreposições). */
export function Card({ as: Tag = "div", className, ...rest }: Props) {
  return <Tag className={cx("rounded-card border border-edge bg-surface card", className)} {...rest} />;
}
