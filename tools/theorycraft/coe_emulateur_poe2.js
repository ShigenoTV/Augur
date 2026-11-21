#!/usr/bin/env node
// Recoupement de la Désécration de Forge (PoE 2) avec l'émulateur de Craft of Exile (beta.craftofexile.com).
//
// Chaque cas (exporté par le test `exporter_cas_desecration` de `forge::atelier`) donne une base, un objet
// Rare de départ, un os, ses Omens et une famille de mods visée. On applique l'os puis on tire les options
// de la révélation N fois dans l'émulateur, et on compare la part d'essais où la famille visée est proposée
// à celle du moteur exact et du modèle du solveur d'Augur.
//
//     CAS=cas.json cargo test -p poe2-core --release --lib exporter_cas_desecration -- --ignored
//     node tools/theorycraft/coe_emulateur_poe2.js cas.json [essais]
//
// Utilise le Chromium et le Playwright préinstallés. Aucune donnée de l'émulateur n'est copiée.
const { chromium } = require('playwright');
const fs = require('fs');

const HANDLERS = { gnawed: 'poe2_desecrate_low', preserved: 'poe2_desecrate_mid', ancient: 'poe2_desecrate_high' };

async function main() {
  const cas = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
  const essais = Number(process.argv[3] || 20000);
  const navigateur = await chromium.launch();
  const page = await navigateur.newPage();
  await page.goto('https://beta.craftofexile.com/?game=poe2', { waitUntil: 'networkidle', timeout: 120000 });
  const resultats = [];
  for (const c of cas) {
    const r = await page.evaluate(([c, essais, HANDLERS]) => {
      const base = coedata.items.entries.find((x) => x.key === c.base);
      const idMod = (k) => DataManager.getEntryFromDict(coedata.mods, k).id;
      const famille = new Set(c.famille);
      const cle = (id) => DataManager.getEntryFromId(coedata.mods, Number(id))?.key;
      Emulator.setModpool(ModsManager.categorizeModpool(ModsManager.getModpool(base.class, true, true)));
      if (Emulator.setSimulationMode) Emulator.setSimulationMode(true);
      Notices.show = () => {};
      Emulator.consumeOmen = () => {};
      const handler = HANDLERS[c.os.split('_')[0]];
      let vus = 0;
      let echecs = 0;
      for (let n = 0; n < essais; n++) {
        const depart = new GameItem(base.id, c.ilvl, null, []);
        depart.rarity = ItemRarity.RARE;
        for (const k of c.depart) depart.addModifier(idMod(k));
        // places libres recalculées pour la rareté Rare (sinon l'émulateur garde celles d'un objet Normal)
        depart.updateItemState(false);
        // l'émulateur travaille sur sa propre copie de l'objet : on la relit après coup
        Emulator.setItem(depart);
        const item = Emulator._item;
        const p = new rollAffixParameters();
        if (c.niveau_min > 0) p._minimumModifierLevel = c.niveau_min;
        const ok = Emulator.poe2_desecrate(handler, p, c.omens.slice());
        const voile = item.getModifiers().find((m) => m.veiled && !m.revealed);
        if (ok !== true || !voile) { echecs++; continue; }
        let choix = Emulator.getRevealPicks(voile.type, voile.params);
        if (voile.params && voile.params.reroll) choix = [...new Set([...choix, ...Emulator.getRevealPicks(voile.type, voile.params)])];
        if (choix.some((id) => famille.has(cle(id)))) vus++;
      }
      return { vus, echecs };
    }, [c, essais, HANDLERS]);
    const coe = r.vus / essais;
    const sigma = Math.sqrt(Math.max(coe * (1 - coe), 1e-12) / essais);
    resultats.push({ nom: c.nom, moteur: c.augur_moteur, modele: c.augur_modele, emulateur: coe, sigma, echecs: r.echecs, essais });
    console.error(
      `${c.nom} : moteur ${(100 * c.augur_moteur).toFixed(2)} %, modèle ${(100 * c.augur_modele).toFixed(2)} %, ` +
        `émulateur ${(100 * coe).toFixed(2)} % ± ${(100 * sigma).toFixed(2)} (échecs ${r.echecs})`,
    );
  }
  console.log(JSON.stringify(resultats, null, 1));
  await navigateur.close();
}

main().catch((e) => { console.error(e); process.exit(1); });
