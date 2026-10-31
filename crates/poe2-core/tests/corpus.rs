//! Parseur d'objets sur le corpus de `tests/corpus/objets/`.

use std::path::{Path, PathBuf};

use poe2_core::parser::{self, Affixe, ErreurParse, GenreMod, Rarete};

fn dossier() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/objets")
}

fn lire(nom: &str) -> String {
    std::fs::read_to_string(dossier().join(nom)).expect(nom)
}

fn objet(nom: &str) -> parser::Objet {
    parser::parser(&lire(nom)).unwrap_or_else(|e| panic!("{nom} : {e}"))
}

#[test]
fn taux_de_reussite_du_corpus() {
    let mut ok = 0usize;
    let mut echecs = Vec::new();
    for e in std::fs::read_dir(dossier()).unwrap().flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("txt") {
            continue;
        }
        let texte = std::fs::read_to_string(&p).unwrap();
        match parser::parser(&texte) {
            Ok(o) if o.lignes_inconnues.is_empty() => ok += 1,
            Ok(o) => echecs.push(format!(
                "{} : lignes inconnues {:?}",
                p.display(),
                o.lignes_inconnues
            )),
            Err(err) => echecs.push(format!("{} : {err}", p.display())),
        }
    }
    let total = ok + echecs.len();
    println!("corpus : {ok}/{total} objets reconnus");
    assert!(total > 0);
    // Objectif du sprint : 99 %. Le corpus actuel doit passer à 100 %.
    assert!(echecs.is_empty(), "{}", echecs.join("\n"));
}

#[test]
fn invalides_refuses() {
    for e in std::fs::read_dir(dossier().join("invalides"))
        .unwrap()
        .flatten()
    {
        let texte = std::fs::read_to_string(e.path()).unwrap();
        assert!(
            parser::parser(&texte).is_err(),
            "{} accepté",
            e.path().display()
        );
    }
    assert_eq!(
        parser::parser(&lire("invalides/client-francais.txt")),
        Err(ErreurParse::LangueNonPriseEnCharge)
    );
    assert_eq!(parser::parser(""), Err(ErreurParse::Vide));
    assert!(matches!(
        parser::parser(&"a".repeat(100_000)),
        Err(ErreurParse::TropLong(_))
    ));
}

#[test]
fn rare_simple() {
    let o = objet("rare-baguette.txt");
    assert_eq!(o.classe.as_deref(), Some("Wands"));
    assert_eq!(o.rarete, Rarete::Rare);
    assert_eq!(o.nom.as_deref(), Some("Doom Song"));
    assert_eq!(o.base, "Withered Wand");
    assert_eq!(o.niveau_objet, Some(79));
    assert_eq!(o.exigences.niveau, Some(52));
    assert_eq!(o.exigences.intelligence, Some(89));
    assert!(o.corrompu);
    assert!(!o.avance);
    let modeles: Vec<_> = o.mods.iter().map(|m| m.modele.as_str()).collect();
    assert_eq!(
        modeles,
        [
            "Grants Skill: Level # Chaos Bolt",
            "Adds # to # Fire Damage to Spells",
            "+# to maximum Mana",
            "#% increased Spell Damage",
            "+# to Level of all Spell Skills",
            "+#% to Fire Resistance",
        ]
    );
    assert_eq!(o.mods[1].valeurs, vec![7.0, 14.0]);
}

#[test]
fn rare_avance() {
    let o = objet("rare-casque-avance.txt");
    assert!(o.avance);
    assert_eq!(o.qualite, Some(20));
    let es = o.propriete("Energy Shield").unwrap();
    assert_eq!(es.valeur, "142");
    assert!(es.augmentee);
    assert_eq!(o.emplacements.as_deref(), Some("S S"));
    assert_eq!(o.mods_de_genre(GenreMod::Rune).count(), 1);
    let explicites: Vec<_> = o.mods_de_genre(GenreMod::Explicite).collect();
    assert_eq!(explicites.len(), 4);
    let vie = explicites[1];
    assert_eq!(vie.texte, "+89 to maximum Life");
    assert_eq!(vie.plages, vec![Some((85.0, 99.0))]);
    let e = vie.entete.as_ref().unwrap();
    assert_eq!(
        (e.affixe, e.nom.as_deref(), e.palier),
        (Affixe::Prefixe, Some("Robust"), Some(2))
    );
    assert_eq!(e.tags, vec!["Life"]);
    let froid = explicites[3];
    assert_eq!(froid.valeurs, vec![-10.0]);
    assert_eq!(froid.plages, vec![Some((-12.0, -8.0))]);
    assert_eq!(froid.entete.as_ref().unwrap().affixe, Affixe::Suffixe);
}

#[test]
fn implicite_avance_et_note() {
    let o = objet("rare-bague-avance.txt");
    let imp: Vec<_> = o.mods_de_genre(GenreMod::Implicite).collect();
    assert_eq!(imp.len(), 1);
    assert_eq!(imp[0].modele, "+#% to Fire Resistance");
    assert_eq!(imp[0].entete.as_ref().unwrap().affixe, Affixe::Implicite);
    assert_eq!(o.note.as_deref(), Some("~price 2 exalted"));
    assert_eq!(o.mods_de_genre(GenreMod::Explicite).count(), 3);
}

#[test]
fn magique_normal() {
    let o = objet("magique-amulette.txt");
    assert_eq!(o.rarete, Rarete::Magique);
    assert_eq!(o.nom, None);
    assert_eq!(o.base, "Glowing Amber Amulet of the Wind");
    assert_eq!(o.mods_de_genre(GenreMod::Implicite).count(), 1);
    assert_eq!(o.mods_de_genre(GenreMod::Explicite).count(), 2);

    let o = objet("normal-bouclier.txt");
    assert_eq!(o.rarete, Rarete::Normal);
    assert_eq!(o.propriete("Armour").unwrap().valeur, "29");
    assert_eq!(o.propriete("Block chance").unwrap().valeur, "25%");
}

#[test]
fn uniques() {
    let o = objet("unique-gants.txt");
    assert_eq!(o.nom.as_deref(), Some("Hand of Ash"));
    assert_eq!(o.exigences.dexterite, Some(21));
    assert_eq!(o.mods.len(), 4);
    assert_eq!(
        o.texte_saveur.as_deref(),
        Some("The fire in his palm\nnever touched his sleeve.")
    );

    let o = objet("unique-arme-avance.txt");
    assert!(o.miroir);
    assert_eq!(o.propriete("Attacks per Second").unwrap().valeur, "1.40");
    assert_eq!(o.mods.len(), 3);
    assert!(
        o.mods
            .iter()
            .all(|m| m.entete.as_ref().unwrap().affixe == Affixe::Unique)
    );
    assert_eq!(o.mods[0].valeurs, vec![1.0, 40.0]);
    assert_eq!(o.mods[0].plages, vec![None, Some((30.0, 50.0))]);
    assert_eq!(o.mods[2].texte, "Lightning Damage from Hits Shocks Enemies");
}

#[test]
fn gemme_devise_non_identifie() {
    let o = objet("gemme.txt");
    assert_eq!(o.rarete, Rarete::Gemme);
    assert_eq!(o.base, "Fireball");
    assert_eq!(o.propriete("Cast Time").unwrap().valeur, "1.20 sec");
    assert_eq!(o.exigences.niveau, Some(30));

    let o = objet("devise.txt");
    assert_eq!(o.rarete, Rarete::Devise);
    assert_eq!(o.taille_pile, Some((7, 20)));

    let o = objet("rare-non-identifie.txt");
    assert!(o.non_identifie);
    assert_eq!(o.nom, None);
    assert_eq!(o.base, "Votive Raiment");
    assert!(o.mods.is_empty());
}

#[test]
fn desacralise_et_drapeaux() {
    let o = objet("rare-desacralise-avance.txt");
    assert!(o.corrompu && o.sanctifie);
    let d: Vec<_> = o.mods_de_genre(GenreMod::Desacralise).collect();
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].entete.as_ref().unwrap().palier, None);
    assert_eq!(
        o.mods[2].entete.as_ref().unwrap().tags,
        Vec::<String>::new()
    );
}

#[test]
fn fins_de_ligne_windows() {
    let o = objet("rare-fins-de-ligne-windows.txt");
    assert_eq!(o.base, "Attuned Wand");
    assert_eq!(o.mods[0].modele, "+# to maximum Mana");
}
