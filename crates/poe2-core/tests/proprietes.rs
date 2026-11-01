//! Tests de propriétés et garde-fous des entrées non fiables.

use poe2_core::calc::golden::{self, Verdict};
use poe2_core::calc::{self, Conditions, Modificateur, Personnage, TypeModificateur};
use poe2_core::parser;
use proptest::prelude::*;

proptest! {
    /// Le presse-papier peut contenir n'importe quoi : jamais de panique.
    #[test]
    fn parseur_ne_panique_jamais(texte in "\\PC{0,400}") {
        let _ = parser::parser(&texte);
    }

    /// Variante « presque un objet » : en-têtes valides, corps aléatoire.
    #[test]
    fn parseur_objet_bruite(corps in "[ -~\\n{}()—-]{0,300}") {
        let texte = format!("Item Class: Rings\nRarity: Rare\nX\nY\n--------\n{corps}");
        let _ = parser::parser(&texte);
    }

    /// Ajouter de l'increased de dégâts de sort n'abaisse jamais le DPS.
    #[test]
    fn dps_monotone(inc in 0.0f64..500.0, res in -50.0f64..75.0) {
        let perso: Personnage = serde_yaml_ng::from_str(
            "competence: { nom: t, temps_incantation: 1.0, critique_base: 5, degats: { feu: [10, 20] } }",
        ).unwrap();
        let mut cond = Conditions::default();
        cond.cible.resistances.insert(calc::TypeDegats::Feu, res);
        let ajout = [Modificateur::new("spell_damage", TypeModificateur::Increased, inc, "test")];
        let (avant, apres) = calc::delta(&perso, &cond, &ajout, "dps").unwrap().unwrap();
        prop_assert!(apres >= avant - 1e-9);
    }
}

const CAS: &str = r#"
id: test
patch: "0.3.0"
confiance: supposé
personnage:
  competence: { nom: t, temps_incantation: 1.0, critique_base: 0, degats: { feu: [10, 10] } }
attendu:
  dps: VALEUR
"#;

#[test]
fn cas_faux_detecte() {
    let cas = golden::lire(&CAS.replace("VALEUR", "11")).unwrap();
    let r = golden::executer(&cas[0]);
    assert_eq!(r.verdict, Verdict::Echec);
    let cas = golden::lire(&CAS.replace("VALEUR", "10")).unwrap();
    assert_eq!(golden::executer(&cas[0]).verdict, Verdict::Reussi);
}

#[test]
fn tooltip_dps_accepte() {
    let cas = golden::lire(&CAS.replace("dps: VALEUR", "tooltip_dps: 10")).unwrap();
    assert_eq!(golden::executer(&cas[0]).verdict, Verdict::Reussi);
}

#[test]
fn sortie_inconnue_signalee() {
    let cas = golden::lire(&CAS.replace("dps: VALEUR", "dsp: 10")).unwrap();
    let r = golden::executer(&cas[0]);
    assert_eq!(r.verdict, Verdict::Echec);
    assert!(r.erreurs[0].contains("dsp"));
}

#[test]
fn stat_inconnue_refusee() {
    let yaml = CAS.replace("VALEUR", "10").replace(
        "attendu:",
        "  modificateurs:\n    - { stat: fire_damgae, type: increased, valeur: 10, source: s }\nattendu:",
    );
    let cas = golden::lire(&yaml).unwrap();
    let r = golden::executer(&cas[0]);
    assert_eq!(r.verdict, Verdict::Erreur);
    assert!(r.erreurs[0].contains("fire_damgae"));
}

#[test]
fn champ_inconnu_refuse() {
    let yaml = CAS
        .replace("VALEUR", "10")
        .replace("confiance:", "confience: x\nconfiance:");
    assert!(golden::lire(&yaml).is_err());
}

#[test]
fn trace_serialisable_et_lisible() {
    let cas = golden::lire(&CAS.replace("VALEUR", "10")).unwrap();
    let r = golden::executer(&cas[0]);
    let t = r.resultats.unwrap().trace("dps").unwrap().clone();
    let json = serde_json::to_string(&t).unwrap();
    assert!(json.contains("\"libelle\""));
    assert!(t.to_string().contains("incantations par seconde"));
}

mod formules {
    use poe2_core::calc::formules::{self, Verdict};

    fn un(yaml: &str) -> Verdict {
        let f = formules::lire(yaml).unwrap();
        formules::executer(&f.cas[0]).verdict
    }

    const ARMURE: &str = "cas:\n  - { id: a, type: unitaire, calcul: armure.reduction, entrees: { armure: 1000, degats: 100 }, attendu: { reduction_pct: VAL } }";

    #[test]
    fn valeur_fausse_en_echec() {
        assert_eq!(un(&ARMURE.replace("VAL", "50")), Verdict::Reussi);
        assert_eq!(un(&ARMURE.replace("VAL", "51")), Verdict::Echec);
    }

    #[test]
    fn entree_inconnue_en_erreur() {
        let y = ARMURE
            .replace("VAL", "50")
            .replace("degats: 100", "degats: 100, armur: 3");
        assert_eq!(un(&y), Verdict::Erreur);
    }

    #[test]
    fn calcul_inconnu_en_erreur() {
        let y = "cas:\n  - { id: x, type: unitaire, calcul: inconnu, attendu: { x: 0 } }";
        assert_eq!(un(y), Verdict::Erreur);
    }

    #[test]
    fn gabarit_personnage_en_attente() {
        let y = "cas:\n  - { id: p, type: personnage, statut: à-mesurer, attendu: null }";
        assert_eq!(un(y), Verdict::EnAttente);
    }
}
