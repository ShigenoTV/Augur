import type { SVGProps } from "react";

const paths = {
  check: "M3.5 8.5l3 3 6-7",
  approx: "M3 6.5c1.5-1.5 3-1.5 5 0s3.5 1.5 5 0M3 10.5c1.5-1.5 3-1.5 5 0s3.5 1.5 5 0",
  ban: "M8 2.5a5.5 5.5 0 1 0 0 11a5.5 5.5 0 0 0 0-11zM4.2 4.2l7.6 7.6",
  up: "M8 3.5l4.5 6h-9z",
  down: "M8 12.5l-4.5-6h9z",
  flat: "M8 6a2 2 0 1 0 0 4a2 2 0 0 0 0-4z",
  clock: "M8 2.5a5.5 5.5 0 1 0 0 11a5.5 5.5 0 0 0 0-11zM8 5v3.2l2.2 1.4",
  alert: "M8 2.5l6 10.5H2zM8 6.5v3M8 11.2v.3",
  info: "M8 2.5a5.5 5.5 0 1 0 0 11a5.5 5.5 0 0 0 0-11zM8 7.2v3.8M8 5v.3",
  user: "M8 3a2.5 2.5 0 1 0 0 5a2.5 2.5 0 0 0 0-5zM3 13.5c.6-2.4 2.6-3.6 5-3.6s4.4 1.2 5 3.6",
  pause: "M5.5 3.5v9M10.5 3.5v9",
  search: "M7 2.8a4.2 4.2 0 1 0 0 8.4a4.2 4.2 0 0 0 0-8.4zM10.1 10.1l3.4 3.4",
} as const;

export type IconName = keyof typeof paths;

/** Pictogrammes au trait 16 px : ils doublent toujours l'information portée par la couleur. */
export function Icon({
  name,
  size = 14,
  ...rest
}: { name: IconName; size?: number } & Omit<SVGProps<SVGSVGElement>, "name">) {
  const filled = name === "up" || name === "down" || name === "flat";
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 16 16"
      aria-hidden="true"
      focusable="false"
      fill={filled ? "currentColor" : "none"}
      stroke={filled ? "none" : "currentColor"}
      strokeWidth={1.6}
      strokeLinecap="round"
      strokeLinejoin="round"
      {...rest}
    >
      <path d={paths[name]} />
    </svg>
  );
}
