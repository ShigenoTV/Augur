use super::*;

const BUILD: &str = include_str!("../tests/donnees/berserker-pob.xml");

const ANNEAU: &str = "Item Class: Rings
Rarity: Rare
Gale Loop
Two-Stone Ring
--------
Requirements:
Level: 60
--------
Item Level: 84
--------
+16% to Fire and Lightning Resistances (implicit)
--------
+72 to maximum Life
+45% to Cold Resistance
+40% to Lightning Resistance
+12% to Chaos Resistance
";

const BOTTES: &str = "Item Class: Boots
Rarity: Rare
Dread Stride
Two-Toned Boots
--------
Armour: 120
Energy Shield: 30
--------
Item Level: 84
--------
+12% to Fire and Lightning Resistances (implicit)
--------
+90 to maximum Life
30% increased Movement Speed
+40% to Cold Resistance
";

/// Un seul moteur pour tous les tests : PoB met quelques secondes à démarrer.
fn moteur() -> &'static Moteur {
    use std::sync::OnceLock;
    static MOTEUR: OnceLock<(tempfile::TempDir, Moteur)> = OnceLock::new();
    let (_, m) = MOTEUR.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        let m = Moteur::demarrer(dir.path().to_path_buf());
        m.charger(BUILD).expect("build chargé");
        (dir, m)
    });
    m
}

#[test]
fn version_du_paquet() {
    assert!(VERSION.contains("arbre"), "{VERSION}");
}

#[test]
fn comprend_le_build() {
    let dir = tempfile::tempdir().unwrap();
    let m = Moteur::demarrer(dir.path().to_path_buf());
    assert_eq!(m.comparer(ANNEAU).unwrap_err(), ErreurPob::SansBuild);
    let info = m.charger(BUILD).unwrap();
    assert_eq!(info.class_name.as_deref(), Some("Marauder"));
    assert_eq!(info.ascendancy.as_deref(), Some("Berserker"));
    assert!(info.life > 1000.0, "{info:?}");
    assert!(info.combined_dps > 0.0, "{info:?}");
    // Arbre 3.13 absent du paquet : recalé sur le dernier.
    assert_ne!(info.tree_version.as_deref(), Some("3_13"));
    assert!(m.est_charge(BUILD));
}

#[test]
fn compare_un_anneau_aux_deux_emplacements() {
    let c = moteur().comparer(ANNEAU).unwrap();
    let noms: Vec<_> = c.slots.iter().map(|s| s.slot.as_str()).collect();
    assert_eq!(noms, ["Ring 1", "Ring 2"]);
    assert!(c.slots.iter().all(|s| s.equipped.is_some()));
    // L'anneau apporte du chaos : la résistance au chaos monte partout.
    for s in &c.slots {
        assert!(
            s.after.resistances.chaos.value > c.before.resistances.chaos.value,
            "{s:?}"
        );
        assert!(!s.lines.is_empty());
    }
    assert!(c.meilleur().is_some());
    assert!(c.before.resistances.fire.cap >= 75.0);
}

#[test]
fn compare_des_bottes() {
    let c = moteur().comparer(BOTTES).unwrap();
    assert_eq!(c.slots.len(), 1);
    assert_eq!(c.slots[0].slot, "Boots");
    assert!(
        c.slots[0].lines.iter().any(|l| l.label.contains("Life")),
        "{:?}",
        c.slots[0].lines
    );
}

#[test]
fn objet_illisible() {
    let e = moteur()
        .comparer("Rarity: Rare\nn'importe quoi")
        .unwrap_err();
    assert!(matches!(e, ErreurPob::Calcul(_)), "{e:?}");
}

#[test]
fn deploiement_reutilise() {
    let dir = tempfile::tempdir().unwrap();
    let a = deployer(dir.path()).unwrap();
    assert!(a.join("src/Launch.lua").exists());
    assert!(a.join("LICENSE.md").exists());
    let b = deployer(dir.path()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn erreurs_sans_couleurs() {
    let e =
        mlua::Error::RuntimeError("^1Error:\n\n^0In 'Init': boum\nstack traceback:\n...".into());
    assert_eq!(texte_erreur(&e), "In 'Init': boum ...");
}
