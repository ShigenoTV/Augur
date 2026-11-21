#!/usr/bin/env node
// Recoupement de la Forge PoE 1 avec l'émulateur de Craft of Exile (beta.craftofexile.com).
//
// Chaque cas (exporté par le test `exporter_cas_emulateur` du solveur PoE 1) donne une base, un
// objet de départ, une action et les mods acceptés pour chaque voulu. On applique l'action N fois
// dans l'émulateur, en mode simulation, et on compare la part de réussites à la chance du modèle.
//
//     CAS=cas.json cargo test -p poe2-core --release --lib exporter_cas_emulateur -- --ignored
//     node tools/theorycraft/coe_emulateur.js cas.json [essais]
//
// Utilise le Chromium et le Playwright préinstallés. Aucune donnée de l'émulateur n'est copiée.
const { chromium } = require('playwright');
const fs = require('fs');

const INFLUENCES = { Shaper: 0, Elder: 1, Crusader: 2, Redeemer: 3, Hunter: 4, Warlord: 5 };

async function main() {
  const cas = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
  const essais = Number(process.argv[3] || 20000);
  const navigateur = await chromium.launch();
  const page = await navigateur.newPage();
  await page.goto('https://beta.craftofexile.com/?game=poe1', { waitUntil: 'networkidle', timeout: 120000 });
  const resultats = [];
  for (const c of cas) {
    const r = await page.evaluate(([c, essais, INFLUENCES]) => {
      const base = coedata.items.entries.find((x) => x.key === c.base);
      const idMod = (k) => DataManager.getEntryFromDict(coedata.mods, k).id;
      const acceptes = c.acceptes.map((l) => new Set(l));
      const influences = (c.influence || '').split('+').filter((x) => x).map((x) => INFLUENCES[x]);
      Emulator.setModpool(ModsManager.categorizeModpool(ModsManager.getModpool(base.class, true, true)));
      if (Emulator.setSimulationMode) Emulator.setSimulationMode(true);
      Notices.show = () => {};
      const [action, tag] = c.action.split(':');
      const harvest = (genre, nom) =>
        coedata.methods.harvest[genre].findIndex((x) => x.name.toLowerCase() === nom);
      let reussites = 0;
      let echecs = 0;
      for (let n = 0; n < essais; n++) {
        const item = new GameItem(base.id, c.ilvl, null, influences);
        Emulator.setItem(item);
        if (c.depart.length > 0 || action !== 'chaos') {
          item.rarity = ItemRarity.RARE;
          for (const k of c.depart) item.addModifier(idMod(k));
        } else {
          Emulator.alchemy(new rollAffixParameters());
        }
        if (c.eldritch === 'eater') { item._eaterInfluence = 6; item._exarchInfluence = 0; }
        if (c.eldritch === 'exarch') { item._eaterInfluence = 0; item._exarchInfluence = 6; }
        let ok;
        const p = new rollAffixParameters();
        switch (action) {
          case 'chaos': ok = Emulator.chaos(p); break;
          case 'eldritch_chaos': ok = Emulator.eldritchChaos(p); break;
          case 'eldritch_exalt': ok = Emulator.eldritchExalt(p); break;
          case 'veiled_chaos': ok = Emulator.veiledChaos(p); break;
          case 'harvest_reforge': ok = Emulator.harvestReforge(['harvest', 'reforge', harvest('reforge', tag)]); break;
          case 'harvest_augment': ok = Emulator.harvestAugment(['harvest', 'augment', harvest('augment', tag)]); break;
        }
        if (ok !== true) { echecs++; continue; }
        let cles = item.getModifiers().map((m) => m.key);
        if (action === 'veiled_chaos') {
          // Dévoilement : réussite si un mod accepté est parmi les trois choix.
          const voile = item.getModifiers().find((m) => m.veiled || /veiled/i.test(m.key || ''));
          if (!voile) { echecs++; continue; }
          const choix = Emulator.getRevealPicks(voile.type).map((id) => DataManager.getEntryFromId(coedata.mods, Number(id))?.key ?? id);
          cles = cles.concat(choix);
        }
        if (acceptes.every((a) => cles.some((k) => a.has(k)))) reussites++;
      }
      return { reussites, echecs };
    }, [c, essais, INFLUENCES]);
    const coe = r.reussites / essais;
    const sigma = Math.sqrt(Math.max(coe * (1 - coe), 1e-12) / essais);
    resultats.push({ nom: c.nom, augur: c.augur, emulateur: coe, sigma, echecs: r.echecs, essais });
    console.error(`${c.nom} : Augur ${(100 * c.augur).toFixed(3)} %, émulateur ${(100 * coe).toFixed(3)} % ± ${(100 * sigma).toFixed(3)} (échecs ${r.echecs})`);
  }
  console.log(JSON.stringify(resultats, null, 1));
  await navigateur.close();
}

main().catch((e) => { console.error(e); process.exit(1); });
