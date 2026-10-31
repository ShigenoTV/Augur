/** Concatène des classes en ignorant les valeurs falsy. */
export function cx(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}
