import type { ItemMod, ModKind, ParsedItem, Rarity } from "../types";
import { cx } from "../lib/cx";

const rarityText: Record<Rarity, string> = {
  normal: "text-rarity-normal",
  magic: "text-rarity-magic",
  rare: "text-rarity-rare",
  unique: "text-rarity-unique-text",
  currency: "text-rarity-currency",
  gem: "text-rarity-gem",
  other: "text-text-primary",
};

const rarityBorder: Record<Rarity, string> = {
  normal: "border-rarity-normal/50",
  magic: "border-rarity-magic/50",
  rare: "border-rarity-rare/50",
  unique: "border-rarity-unique/70",
  currency: "border-rarity-currency/50",
  gem: "border-rarity-gem/50",
  other: "border-ink-600",
};

const modText: Record<ModKind, string> = {
  implicit: "text-mod-implicit",
  explicit: "text-mod-explicit",
  rune: "text-mod-rune",
  enchant: "text-mod-enchant",
  crafted: "text-mod-crafted",
  fractured: "text-mod-fractured",
  desecrated: "text-mod-desecrated",
};

const modTag: Partial<Record<ModKind, string>> = {
  implicit: "implicite",
  rune: "rune",
  enchant: "enchant.",
  crafted: "crafté",
  fractured: "fracturé",
  desecrated: "désacralisé",
};

/** En-tête d'objet : nom coloré par rareté, base et niveau d'objet. */
export function ItemHeader({ item, className }: { item: ParsedItem; className?: string }) {
  const title = item.name ?? item.baseType;
  const showBase = Boolean(item.name) && item.name !== item.baseType;
  return (
    <header className={cx("min-w-0", className)}>
      <h2
        className={cx(
          "truncate font-display text-lg font-semibold [font-variant-caps:small-caps]",
          rarityText[item.rarity],
        )}
        title={title}
      >
        {title}
      </h2>
      <p className="flex items-center gap-2 text-xs text-text-muted">
        {showBase && <span className="truncate">{item.baseType}</span>}
        {item.itemLevel !== null && (
          <span className="shrink-0 text-text-subtle">ilvl {item.itemLevel}</span>
        )}
        {item.unidentified && <span className="shrink-0 text-text-subtle">non identifié</span>}
        {item.corrupted && (
          <span className="shrink-0 font-semibold uppercase text-mod-corrupted">Corrompu</span>
        )}
      </p>
    </header>
  );
}

function ModLine({ mod }: { mod: ItemMod }) {
  const tag = modTag[mod.kind];
  return (
    <li className={cx("flex items-baseline gap-2 text-sm", modText[mod.kind])}>
      <span className="min-w-0 flex-1">{mod.text}</span>
      {(tag || mod.tier !== null || mod.affix !== null) && (
        <span className="shrink-0 text-2xs text-text-subtle">
          {[tag, mod.affix === "prefix" ? "préfixe" : mod.affix === "suffix" ? "suffixe" : null, mod.tier !== null ? `T${mod.tier}` : null]
            .filter(Boolean)
            .join(" · ")}
        </span>
      )}
    </li>
  );
}

/**
 * Tooltip d'objet fidèle aux conventions du jeu, partagé par l'overlay et le
 * planificateur. Les implicites sont séparés des explicites, comme en jeu.
 */
export function ItemTooltip({ item, className }: { item: ParsedItem; className?: string }) {
  const implicits = item.mods.filter((m) => m.kind === "implicit" || m.kind === "enchant" || m.kind === "rune");
  const explicits = item.mods.filter((m) => !implicits.includes(m));
  return (
    <section
      aria-label={`Objet : ${item.name ?? item.baseType}`}
      className={cx("rounded-panel border bg-ink-900/80 p-3", rarityBorder[item.rarity], className)}
    >
      <ItemHeader item={item} />
      {implicits.length > 0 && (
        <>
          <div className="augur-divider my-2" />
          <ul className="space-y-0.5">
            {implicits.map((m, i) => (
              <ModLine key={`i${i}`} mod={m} />
            ))}
          </ul>
        </>
      )}
      {explicits.length > 0 && (
        <>
          <div className="augur-divider my-2" />
          <ul className="space-y-0.5">
            {explicits.map((m, i) => (
              <ModLine key={`e${i}`} mod={m} />
            ))}
          </ul>
        </>
      )}
    </section>
  );
}
