//! Personnage « cible » : l'écart entre le build actuel et le build visé,
//! tous deux importés de Path of Building (PoE 1 ou PoE 2).
//!
//! « Où j'en suis, où je vais » : pour chaque emplacement, l'objet actuel
//! face à l'objet de la cible et les mods qui manquent ; les compétences et
//! supports de la cible absents du build actuel ; les passifs à prendre et à
//! retirer. Les chiffres (DPS, EHP…) sont calculés à part : par le moteur
//! d'Augur en PoE 2 ([`crate::comparaison::chiffres`]), lus dans les stats
//! exportées par Path of Building en PoE 1 ([`chiffres_pob`]).
//!
//! Tout est local et ne dépend que des deux codes collés par le joueur.

use std::collections::{BTreeMap, BTreeSet};

use crate::calc::stats::TypeDegats;
use crate::pob::{BuildPob, ObjetPob};

/// Part de la valeur de la cible au-dessus de laquelle un mod est atteint
/// (un jet un peu plus bas compte comme obtenu).
pub const SEUIL_ATTEINT: f64 = 0.9;

/// Un mod de l'objet cible présent sur l'objet actuel, mais plus faible.
#[derive(Debug, Clone, PartialEq)]
pub struct ModPlusFaible {
    pub cible: String,
    pub actuel: String,
}

/// Où en est un emplacement par rapport à la cible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Statut {
    /// Objet de la cible obtenu (même unique, ou tous ses mods).
    Atteint,
    /// Objet en place, mais des mods manquent ou sont plus faibles.
    AAmeliorer,
    /// La cible porte un unique, l'actuel est un autre objet.
    AutreObjet,
    /// Rien d'équipé à cet emplacement.
    Vide,
}

/// Un emplacement de la cible comparé au build actuel.
#[derive(Debug, Clone, PartialEq)]
pub struct EcartObjet {
    /// Emplacement PoB (« Helmet », « Ring 1 »…).
    pub emplacement: String,
    pub actuel: Option<String>,
    pub cible: Option<String>,
    pub cible_unique: bool,
    pub statut: Statut,
    /// Nombre de mods de l'objet cible (0 pour un unique).
    pub mods_cible: usize,
    pub mods_manquants: Vec<String>,
    pub mods_plus_faibles: Vec<ModPlusFaible>,
}

/// Un support de la cible absent du groupe de la même compétence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupportManquant {
    pub competence: String,
    pub support: String,
}

/// L'écart complet, hors chiffres.
#[derive(Debug, Clone, PartialEq)]
pub struct Ecart {
    /// Classe (ou ascendance) différente : l'arbre n'est pas comparable.
    pub autre_classe: bool,
    pub objets: Vec<EcartObjet>,
    pub competences_manquantes: Vec<String>,
    pub supports_manquants: Vec<SupportManquant>,
    pub passifs_a_prendre: usize,
    pub passifs_a_retirer: usize,
    pub passifs_actuels: usize,
    pub passifs_cible: usize,
}

/// Écart entre le build actuel et la cible.
pub fn ecart(actuel: &BuildPob, cible: &BuildPob) -> Ecart {
    let pa: BTreeSet<u32> = actuel.passifs.iter().copied().collect();
    let pc: BTreeSet<u32> = cible.passifs.iter().copied().collect();
    let (competences_manquantes, supports_manquants) = gemmes(actuel, cible);
    Ecart {
        autre_classe: actuel.classe != cible.classe || actuel.ascendance != cible.ascendance,
        objets: objets(actuel, cible),
        competences_manquantes,
        supports_manquants,
        passifs_a_prendre: pc.difference(&pa).count(),
        passifs_a_retirer: pa.difference(&pc).count(),
        passifs_actuels: pa.len(),
        passifs_cible: pc.len(),
    }
}

// --- Objets ---------------------------------------------------------------

/// Emplacements ignorés : sets d'armes de rechange.
fn compte(emplacement: &str) -> bool {
    !emplacement.ends_with("Swap")
}

/// Ordre d'affichage des emplacements (ceux qui ne sont pas listés à la fin).
fn rang(emplacement: &str) -> usize {
    const ORDRE: [&str; 13] = [
        "Weapon 1",
        "Weapon 2",
        "Helmet",
        "Body Armour",
        "Gloves",
        "Boots",
        "Amulet",
        "Ring 1",
        "Ring 2",
        "Ring 3",
        "Belt",
        "Flask",
        "Charm",
    ];
    ORDRE
        .iter()
        .position(|o| emplacement.starts_with(o))
        .unwrap_or(ORDRE.len())
}

fn objets(actuel: &BuildPob, cible: &BuildPob) -> Vec<EcartObjet> {
    let mut emplacements: Vec<&str> = cible
        .objets
        .iter()
        .map(|o| o.emplacement.as_str())
        .filter(|e| compte(e))
        .collect();
    emplacements.sort_by_key(|e| (rang(e), e.to_string()));
    emplacements.dedup();

    let mut ecarts: Vec<EcartObjet> = emplacements
        .iter()
        .filter_map(|e| Some(comparer_objet(e, actuel.objet(e), cible.objet(e)?)))
        .collect();

    // Les deux bagues sont interchangeables : on garde l'appariement qui
    // laisse le moins de choses à faire.
    if let (Some(c1), Some(c2)) = (cible.objet("Ring 1"), cible.objet("Ring 2")) {
        let (a1, a2) = (actuel.objet("Ring 1"), actuel.objet("Ring 2"));
        let droit = [
            comparer_objet("Ring 1", a1, c1),
            comparer_objet("Ring 2", a2, c2),
        ];
        let croise = [
            comparer_objet("Ring 1", a2, c1),
            comparer_objet("Ring 2", a1, c2),
        ];
        let reste = |v: &[EcartObjet; 2]| v.iter().map(cout).sum::<usize>();
        if reste(&croise) < reste(&droit) {
            for e in croise {
                if let Some(x) = ecarts.iter_mut().find(|x| x.emplacement == e.emplacement) {
                    *x = e;
                }
            }
        }
    }
    ecarts
}

/// Ce qu'il reste à faire sur un emplacement, pour choisir l'appariement des bagues.
fn cout(e: &EcartObjet) -> usize {
    match e.statut {
        Statut::Atteint => 0,
        Statut::AAmeliorer => e.mods_manquants.len() * 2 + e.mods_plus_faibles.len(),
        Statut::AutreObjet | Statut::Vide => 100,
    }
}

fn unique(o: &ObjetPob) -> bool {
    o.rarete
        .as_deref()
        .is_some_and(|r| r.eq_ignore_ascii_case("UNIQUE") || r.eq_ignore_ascii_case("RELIC"))
}

fn comparer_objet(emplacement: &str, actuel: Option<&ObjetPob>, cible: &ObjetPob) -> EcartObjet {
    let cible_unique = unique(cible);
    let mut e = EcartObjet {
        emplacement: emplacement.to_string(),
        actuel: actuel.and_then(nom_objet),
        cible: nom_objet(cible),
        cible_unique,
        statut: Statut::Vide,
        mods_cible: 0,
        mods_manquants: Vec::new(),
        mods_plus_faibles: Vec::new(),
    };
    let Some(actuel) = actuel else {
        if !cible_unique {
            e.mods_cible = mods(cible).len();
            e.mods_manquants = mods(cible).into_iter().map(|m| m.texte).collect();
        }
        return e;
    };
    if cible_unique {
        e.statut = if nom_objet(actuel) == nom_objet(cible) {
            Statut::Atteint
        } else {
            Statut::AutreObjet
        };
        return e;
    }
    let voulus = mods(cible);
    let presents = mods(actuel);
    e.mods_cible = voulus.len();
    for v in &voulus {
        match presents.iter().find(|p| p.modele == v.modele) {
            None => e.mods_manquants.push(v.texte.clone()),
            Some(p) if !atteint(&p.valeurs, &v.valeurs) => {
                e.mods_plus_faibles.push(ModPlusFaible {
                    cible: v.texte.clone(),
                    actuel: p.texte.clone(),
                })
            }
            Some(_) => {}
        }
    }
    e.statut = if e.mods_manquants.is_empty() && e.mods_plus_faibles.is_empty() {
        Statut::Atteint
    } else {
        Statut::AAmeliorer
    };
    e
}

/// Nom affiché d'un objet : son nom, et sa base pour un rare (« Doom Song, Withered Wand »).
fn nom_objet(o: &ObjetPob) -> Option<String> {
    match (&o.nom, &o.base) {
        (Some(n), Some(b)) if n != b && !unique(o) => Some(format!("{n}, {b}")),
        (Some(n), _) => Some(n.clone()),
        (None, b) => b.clone(),
    }
}

/// Toutes les valeurs atteignent (à [`SEUIL_ATTEINT`] près) celles de la cible.
/// Une valeur négative (malus) est atteinte quand elle n'est pas pire.
fn atteint(actuel: &[f64], cible: &[f64]) -> bool {
    actuel.len() == cible.len()
        && actuel.iter().zip(cible).all(|(a, c)| {
            if *c >= 0.0 {
                *a >= c * SEUIL_ATTEINT - 1e-9
            } else {
                *a >= *c - 1e-9
            }
        })
}

/// Un mod d'objet : son texte (valeurs comprises), son modèle (« +# to
/// maximum Life ») et ses valeurs.
#[derive(Debug, Clone, PartialEq)]
struct Mod {
    texte: String,
    /// Clé de comparaison (« # to maximum Life » : « +# » et « # » confondus).
    modele: String,
    /// Forme affichée (« +# to maximum Life »), pour réécrire une somme.
    forme: String,
    valeurs: Vec<f64>,
}

/// Lignes d'en-tête ou d'état qui ne sont pas des mods.
fn ligne_ignoree(l: &str) -> bool {
    matches!(
        l,
        "Corrupted" | "Mirrored" | "Unidentified" | "Split" | "Sanctified" | "Fractured Item"
    ) || l.starts_with("Requires ")
}

/// Mods d'un objet PoB (implicites et explicites), un modèle par mod : deux
/// lignes de même modèle sont additionnées.
fn mods(o: &ObjetPob) -> Vec<Mod> {
    let lignes: Vec<&str> = o.texte.lines().collect();
    let variante = lignes
        .iter()
        .find_map(|l| l.strip_prefix("Selected Variant:"))
        .and_then(|v| v.trim().parse::<u32>().ok());
    let debut = lignes
        .iter()
        .position(|l| l.starts_with("Implicits:"))
        .map_or(0, |i| i + 1);
    let mut par_modele: BTreeMap<String, Mod> = BTreeMap::new();
    let mut ordre: Vec<String> = Vec::new();
    for brute in &lignes[debut..] {
        let Some(texte) = nettoyer(brute, variante) else {
            continue;
        };
        if texte.is_empty() || ligne_ignoree(&texte) {
            continue;
        }
        let (forme, valeurs) = modele(&texte);
        let modele = forme.replace("+#", "#");
        match par_modele.get_mut(&modele) {
            Some(m) if m.valeurs.len() == valeurs.len() => {
                for (a, b) in m.valeurs.iter_mut().zip(&valeurs) {
                    *a += b;
                }
                m.texte = remplir(&m.forme, &m.valeurs);
            }
            Some(_) => {}
            None => {
                ordre.push(modele.clone());
                par_modele.insert(
                    modele.clone(),
                    Mod {
                        texte,
                        modele,
                        forme,
                        valeurs,
                    },
                );
            }
        }
    }
    ordre
        .into_iter()
        .filter_map(|m| par_modele.remove(&m))
        .collect()
}

/// Retire les balises PoB d'une ligne (`{crafted}`, `{range:0.5}`…) et
/// résout ses plages (`(70-89)`). `None` si la ligne appartient à une autre
/// variante de l'unique.
fn nettoyer(ligne: &str, variante: Option<u32>) -> Option<String> {
    let mut reste = ligne.trim();
    let mut plage = 0.5;
    while let Some(fin) = reste.strip_prefix('{').and_then(|r| r.find('}')) {
        let balise = &reste[1..=fin];
        if let Some(v) = balise.strip_prefix("range:") {
            plage = v.trim().parse::<f64>().unwrap_or(0.5).clamp(0.0, 1.0);
        }
        if let (Some(v), Some(choisie)) = (balise.strip_prefix("variant:"), variante)
            && !v
                .split(',')
                .any(|x| x.trim().parse::<u32>().ok() == Some(choisie))
        {
            return None;
        }
        reste = reste[fin + 2..].trim_start();
    }
    Some(resoudre_plages(reste, plage))
}

/// `(a-b)` devient `a + (b-a) × plage`, sauf juste après un chiffre
/// (« +28(25-30)% » : la valeur est déjà là, la plage est retirée).
fn resoudre_plages(texte: &str, plage: f64) -> String {
    let mut sortie = String::with_capacity(texte.len());
    let mut reste = texte;
    while let Some(debut) = reste.find('(') {
        let Some(fin) = reste[debut..].find(')').map(|f| debut + f) else {
            break;
        };
        let dedans = &reste[debut + 1..fin];
        let bornes = dedans.split_once('-').and_then(|(a, b)| {
            Some((a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?))
        });
        let Some((a, b)) = bornes else {
            sortie.push_str(&reste[..=fin]);
            reste = &reste[fin + 1..];
            continue;
        };
        sortie.push_str(&reste[..debut]);
        if !sortie.ends_with(|c: char| c.is_ascii_digit()) {
            let v = a + (b - a) * plage;
            let entier = a.fract() == 0.0 && b.fract() == 0.0;
            sortie.push_str(&if entier {
                format!("{}", v.floor())
            } else {
                format!("{}", (v * 100.0).round() / 100.0)
            });
        }
        reste = &reste[fin + 1..];
    }
    sortie.push_str(reste);
    sortie
}

/// Forme d'une ligne (nombres remplacés par `#`, signe « + » gardé) et ses valeurs.
fn modele(texte: &str) -> (String, Vec<f64>) {
    let mut modele = String::with_capacity(texte.len());
    let mut valeurs = Vec::new();
    let mut nombre = String::new();
    let vider = |nombre: &mut String, modele: &mut String, valeurs: &mut Vec<f64>| {
        if !nombre.is_empty() {
            if let Ok(v) = nombre.parse::<f64>() {
                // « -7% to Chaos Resistance » : le signe fait partie de la valeur.
                let negatif = modele.ends_with('-');
                if negatif {
                    modele.pop();
                }
                valeurs.push(if negatif { -v } else { v });
                modele.push('#');
            } else {
                modele.push_str(nombre);
            }
            nombre.clear();
        }
    };
    for c in texte.chars() {
        if c.is_ascii_digit() || (c == '.' && !nombre.is_empty()) {
            nombre.push(c);
        } else {
            vider(&mut nombre, &mut modele, &mut valeurs);
            modele.push(c);
        }
    }
    vider(&mut nombre, &mut modele, &mut valeurs);
    (modele, valeurs)
}

/// Texte d'un mod à partir de son modèle et de ses valeurs (mods additionnés).
fn remplir(modele: &str, valeurs: &[f64]) -> String {
    let mut sortie = String::new();
    let mut it = valeurs.iter();
    for c in modele.chars() {
        if c == '#' {
            let v = it.next().copied().unwrap_or(0.0);
            let v = (v * 100.0).round() / 100.0;
            if v < 0.0 && sortie.ends_with('+') {
                sortie.pop();
            }
            sortie.push_str(&format!("{v}"));
        } else {
            sortie.push(c);
        }
    }
    sortie
}

// --- Objet copié en jeu (overlay, Alt+U) ----------------------------------

/// Emplacements PoB possibles pour une classe d'objet du jeu (PoE 1 et PoE 2).
/// Flacons, charmes et joyaux : `None` (non suivis).
pub fn emplacements(classe: &str) -> Option<Vec<&'static str>> {
    if let Some((choix, _)) = crate::comparaison::emplacements(classe) {
        return Some(choix.to_vec());
    }
    if classe.starts_with("Two Hand") || classe == "Fishing Rods" {
        return Some(vec!["Weapon 1"]);
    }
    if classe.starts_with("One Hand") || classe.starts_with("Thrusting") || classe == "Rune Daggers"
    {
        return Some(vec!["Weapon 1", "Weapon 2"]);
    }
    None
}

/// Où en est l'objet copié par rapport à l'objet de la cible pour son emplacement.
#[derive(Debug, Clone, PartialEq)]
pub struct Progression {
    pub emplacement: String,
    pub cible: Option<String>,
    pub cible_unique: bool,
    pub statut: Statut,
    /// Mods de l'objet cible atteints par l'objet copié (unique : 1 si c'est lui).
    pub atteints: usize,
    /// Mods de l'objet cible (unique : 1).
    pub total: usize,
    pub mods_manquants: Vec<String>,
    pub mods_plus_faibles: Vec<ModPlusFaible>,
    /// Même décompte pour l'objet porté à cet emplacement (`None` : rien d'équipé
    /// ou pas de personnage actuel).
    pub actuel_atteints: Option<usize>,
}

fn atteints(e: &EcartObjet) -> (usize, usize) {
    if e.cible_unique {
        (usize::from(e.statut == Statut::Atteint), 1)
    } else {
        (
            e.mods_cible
                .saturating_sub(e.mods_manquants.len() + e.mods_plus_faibles.len()),
            e.mods_cible,
        )
    }
}

/// Objet copié en jeu, comparé à l'objet de la cible. `rarete` : « RARE »,
/// « UNIQUE »… ; `lignes` : mods tels qu'affichés (sans plages ni suffixes).
/// Pour les anneaux, l'emplacement où l'objet est le plus proche de la cible.
pub fn progression(
    actuel: Option<&BuildPob>,
    cible: &BuildPob,
    classe: &str,
    rarete: &str,
    nom: Option<&str>,
    base: &str,
    lignes: &[String],
) -> Option<Progression> {
    let mut texte = format!("Rarity: {rarete}\n");
    if let Some(n) = nom {
        texte.push_str(n);
        texte.push('\n');
    }
    texte.push_str(base);
    texte.push_str("\nImplicits: 0");
    for l in lignes {
        texte.push('\n');
        texte.push_str(l);
    }
    let copie = ObjetPob {
        emplacement: String::new(),
        rarete: Some(rarete.to_string()),
        nom: Some(nom.unwrap_or(base).to_string()),
        base: Some(base.to_string()),
        texte,
    };
    emplacements(classe)?
        .into_iter()
        .filter_map(|e| {
            let objet_cible = cible.objet(e)?;
            let ecart = comparer_objet(e, Some(&copie), objet_cible);
            let actuel_atteints = actuel
                .and_then(|a| a.objet(e))
                .map(|o| atteints(&comparer_objet(e, Some(o), objet_cible)).0);
            Some((ecart, actuel_atteints))
        })
        .min_by_key(|(e, _)| cout(e))
        .map(|(e, actuel_atteints)| {
            let (a, total) = atteints(&e);
            Progression {
                emplacement: e.emplacement,
                cible: e.cible,
                cible_unique: e.cible_unique,
                statut: e.statut,
                atteints: a,
                total,
                mods_manquants: e.mods_manquants,
                mods_plus_faibles: e.mods_plus_faibles,
                actuel_atteints,
            }
        })
}

/// L'objet de la cible pour un emplacement, de quoi le chercher sur le site de trade.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjetCible {
    pub emplacement: String,
    /// Nom de l'unique (`None` pour un rare ou un magique).
    pub unique: Option<String>,
    pub base: Option<String>,
    /// Mods explicites (sans implicites, enchantements ni runes), plages résolues.
    pub lignes: Vec<String>,
}

/// Objet de la cible à l'emplacement `emplacement`.
pub fn objet_cible(cible: &BuildPob, emplacement: &str) -> Option<ObjetCible> {
    let o = cible.objet(emplacement)?;
    let lignes: Vec<&str> = o.texte.lines().collect();
    let variante = lignes
        .iter()
        .find_map(|l| l.strip_prefix("Selected Variant:"))
        .and_then(|v| v.trim().parse::<u32>().ok());
    let (debut, implicites) = lignes
        .iter()
        .position(|l| l.starts_with("Implicits:"))
        .map_or((0, 0), |i| {
            let n = lignes[i]["Implicits:".len()..].trim().parse().unwrap_or(0);
            (i + 1, n)
        });
    let explicites = lignes[debut..]
        .iter()
        .skip(implicites)
        .filter(|l| {
            let t = l.trim_start();
            !(t.starts_with("{enchant}") || t.starts_with("{rune}") || t.starts_with("{implicit}"))
        })
        .filter_map(|l| nettoyer(l, variante))
        .filter(|l| !l.is_empty() && !ligne_ignoree(l))
        .collect();
    let base = o.base.clone().or_else(|| o.nom.clone());
    Some(ObjetCible {
        emplacement: emplacement.to_string(),
        unique: unique(o).then(|| o.nom.clone()).flatten(),
        base,
        lignes: explicites,
    })
}

// --- Gemmes ---------------------------------------------------------------

fn cle(nom: &str, skill_id: &str) -> String {
    if skill_id.is_empty() {
        nom.to_lowercase()
    } else {
        skill_id.to_string()
    }
}

fn gemmes(actuel: &BuildPob, cible: &BuildPob) -> (Vec<String>, Vec<SupportManquant>) {
    // Compétence actuelle → supports de son groupe.
    let mut groupes_actuels: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for gr in actuel.groupes.iter().filter(|g| g.actif) {
        let supports: BTreeSet<String> = gr.supports().map(|s| cle(&s.nom, &s.skill_id)).collect();
        for a in gr.actives() {
            groupes_actuels
                .entry(cle(&a.nom, &a.skill_id))
                .or_default()
                .extend(supports.iter().cloned());
        }
    }
    let mut competences = Vec::new();
    let mut supports = Vec::new();
    let mut vues = BTreeSet::new();
    for gr in cible.groupes.iter().filter(|g| g.actif) {
        for a in gr.actives() {
            let k = cle(&a.nom, &a.skill_id);
            if !vues.insert(k.clone()) {
                continue;
            }
            match groupes_actuels.get(&k) {
                None => competences.push(a.nom.clone()),
                Some(presents) => {
                    for s in gr.supports() {
                        if !presents.contains(&cle(&s.nom, &s.skill_id)) {
                            supports.push(SupportManquant {
                                competence: a.nom.clone(),
                                support: s.nom.clone(),
                            });
                        }
                    }
                }
            }
        }
    }
    (competences, supports)
}

// --- Chiffres lus dans l'export de Path of Building ------------------------

/// Chiffres d'un build, tels que Path of Building les a exportés (`<PlayerStat>`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ChiffresPob {
    pub dps: Option<f64>,
    pub vie: Option<f64>,
    pub bouclier_energie: Option<f64>,
    /// Moyenne des plus gros coups encaissables des cinq types (même
    /// définition que l'overlay), si PoB les a exportés.
    pub ehp: Option<f64>,
    /// Feu, froid, foudre, chaos.
    pub resistances: Vec<(TypeDegats, f64)>,
}

/// Lit les chiffres exportés par Path of Building. DPS : « Full DPS » s'il
/// est calculé, sinon le DPS combiné, sinon le DPS total de la compétence.
pub fn chiffres_pob(stats: &BTreeMap<String, f64>) -> ChiffresPob {
    let get = |k: &str| stats.get(k).copied().filter(|v| v.is_finite());
    let positif = |k: &str| get(k).filter(|v| *v > 0.0);
    let coups: Vec<f64> = ["Physical", "Fire", "Cold", "Lightning", "Chaos"]
        .iter()
        .filter_map(|t| positif(&format!("{t}MaximumHitTaken")))
        .collect();
    let resistances = [
        (TypeDegats::Feu, "Fire"),
        (TypeDegats::Froid, "Cold"),
        (TypeDegats::Foudre, "Lightning"),
        (TypeDegats::Chaos, "Chaos"),
    ]
    .iter()
    .filter_map(|(t, n)| get(&format!("{n}Resist")).map(|v| (*t, v)))
    .collect();
    ChiffresPob {
        dps: positif("FullDPS")
            .or_else(|| positif("CombinedDPS"))
            .or_else(|| positif("TotalDPS")),
        vie: get("Life"),
        bouclier_energie: get("EnergyShield"),
        ehp: (coups.len() == 5).then(|| coups.iter().sum::<f64>() / 5.0),
        resistances,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pob;

    fn build(classe: &str, groupes: &str, objets: &str, passifs: &str) -> BuildPob {
        let xml = format!(
            r#"<PathOfBuilding2><Build level="90" className="{classe}" ascendClassName="None" mainSocketGroup="1"/>
<Tree activeSpec="1"><Spec nodes="{passifs}" treeVersion="0_5"/></Tree>
<Skills activeSkillSet="1"><SkillSet id="1">{groupes}</SkillSet></Skills>
<Items activeItemSet="1">{objets}</Items></PathOfBuilding2>"#
        );
        pob::lire_xml(&xml).unwrap()
    }

    fn item(id: u32, texte: &str) -> String {
        format!("<Item id=\"{id}\">{texte}</Item>")
    }

    fn set(slots: &[(&str, u32)]) -> String {
        let s: String = slots
            .iter()
            .map(|(n, i)| format!(r#"<Slot name="{n}" itemId="{i}"/>"#))
            .collect();
        format!(r#"<ItemSet id="1">{s}</ItemSet>"#)
    }

    const CASQUE_ACTUEL: &str = "Rarity: RARE\nGale Crown\nVoltaic Helm\nImplicits: 0\n+65 to maximum Life\n+30% to Fire Resistance\n+12% to Cold Resistance";
    const CASQUE_CIBLE: &str = "Rarity: RARE\nDoom Visor\nVoltaic Helm\nImplicits: 0\n{crafted}+80 to maximum Life\n{range:0.5}+(30-34)% to Fire Resistance\n+40% to Cold Resistance\n+35% to Lightning Resistance";

    #[test]
    fn mods_manquants_et_plus_faibles() {
        let a = build(
            "Sorceress",
            "",
            &(item(1, CASQUE_ACTUEL) + &set(&[("Helmet", 1)])),
            "",
        );
        let c = build(
            "Sorceress",
            "",
            &(item(1, CASQUE_CIBLE) + &set(&[("Helmet", 1)])),
            "",
        );
        let e = ecart(&a, &c);
        assert_eq!(e.objets.len(), 1);
        let o = &e.objets[0];
        assert_eq!(o.emplacement, "Helmet");
        assert_eq!(o.statut, Statut::AAmeliorer);
        assert_eq!(o.mods_cible, 4);
        assert_eq!(o.mods_manquants, vec!["+35% to Lightning Resistance"]);
        // 65 < 80 × 0,9 ; 30 ≥ 32 × 0,9 (plage résolue à 32) ; 12 < 40 × 0,9.
        assert_eq!(
            o.mods_plus_faibles,
            vec![
                ModPlusFaible {
                    cible: "+80 to maximum Life".into(),
                    actuel: "+65 to maximum Life".into()
                },
                ModPlusFaible {
                    cible: "+40% to Cold Resistance".into(),
                    actuel: "+12% to Cold Resistance".into()
                },
            ]
        );
        assert_eq!(o.actuel.as_deref(), Some("Gale Crown, Voltaic Helm"));
    }

    #[test]
    fn unique_et_emplacement_vide() {
        let tete =
            "Rarity: UNIQUE\nCrown of Eyes\nHubris Circlet\nImplicits: 0\n+50 to maximum Life";
        let a = build(
            "Sorceress",
            "",
            &(item(1, tete) + &set(&[("Helmet", 1)])),
            "",
        );
        let c = build(
            "Sorceress",
            "",
            &(item(1, tete) + &item(2, CASQUE_CIBLE) + &set(&[("Helmet", 1), ("Gloves", 2)])),
            "",
        );
        let e = ecart(&a, &c);
        let casque = e.objets.iter().find(|o| o.emplacement == "Helmet").unwrap();
        assert_eq!(casque.statut, Statut::Atteint);
        assert!(casque.cible_unique);
        let gants = e.objets.iter().find(|o| o.emplacement == "Gloves").unwrap();
        assert_eq!(gants.statut, Statut::Vide);
        assert_eq!(gants.mods_manquants.len(), 4);
        // Ordre d'affichage : casque avant gants.
        assert_eq!(e.objets[0].emplacement, "Helmet");
    }

    #[test]
    fn autre_unique() {
        let a = build(
            "Sorceress",
            "",
            &(item(1, CASQUE_ACTUEL) + &set(&[("Helmet", 1)])),
            "",
        );
        let c = build(
            "Sorceress",
            "",
            &(item(
                1,
                "Rarity: UNIQUE\nCrown of Eyes\nHubris Circlet\nImplicits: 0\n+50 to maximum Life",
            ) + &set(&[("Helmet", 1)])),
            "",
        );
        let o = &ecart(&a, &c).objets[0];
        assert_eq!(o.statut, Statut::AutreObjet);
        assert_eq!(o.cible.as_deref(), Some("Crown of Eyes"));
        assert!(o.mods_manquants.is_empty());
    }

    #[test]
    fn bagues_interchangeables() {
        let vie = "Rarity: RARE\nA\nRuby Ring\nImplicits: 0\n+70 to maximum Life";
        let mana = "Rarity: RARE\nB\nSapphire Ring\nImplicits: 0\n+60 to maximum Mana";
        let a = build(
            "Sorceress",
            "",
            &(item(1, mana) + &item(2, vie) + &set(&[("Ring 1", 1), ("Ring 2", 2)])),
            "",
        );
        let c = build(
            "Sorceress",
            "",
            &(item(1, vie) + &item(2, mana) + &set(&[("Ring 1", 1), ("Ring 2", 2)])),
            "",
        );
        let e = ecart(&a, &c);
        assert!(
            e.objets.iter().all(|o| o.statut == Statut::Atteint),
            "{e:?}"
        );
    }

    #[test]
    fn variantes_d_unique_et_lignes_sommees() {
        let texte = "Rarity: RARE\nX\nY\nSelected Variant: 2\nImplicits: 1\n+10 to maximum Life\n{variant:1}+5% to Fire Resistance\n{variant:2,3}+7% to Cold Resistance\n+20 to maximum Life\n-7% to Chaos Resistance\nCorrupted";
        let o = ObjetPob {
            emplacement: "Belt".into(),
            rarete: Some("RARE".into()),
            nom: Some("X".into()),
            base: Some("Y".into()),
            texte: texte.into(),
        };
        let m = mods(&o);
        let textes: Vec<&str> = m.iter().map(|m| m.texte.as_str()).collect();
        assert_eq!(
            textes,
            vec![
                "+30 to maximum Life",
                "+7% to Cold Resistance",
                "-7% to Chaos Resistance"
            ]
        );
        assert_eq!(m[2].valeurs, vec![-7.0]);
    }

    #[test]
    fn plage_deja_resolue_par_le_jeu() {
        assert_eq!(
            resoudre_plages("+28(25-30)% to Fire Resistance", 0.5),
            "+28% to Fire Resistance"
        );
        assert_eq!(
            resoudre_plages("Adds (10-20) to (30-40) Fire Damage", 1.0),
            "Adds 20 to 40 Fire Damage"
        );
    }

    #[test]
    fn competences_supports_et_passifs() {
        let arc = r#"<Skill enabled="true"><Gem nameSpec="Arc" skillId="ArcPlayer" level="20"/><Gem nameSpec="Zenith II" skillId="SupportZenithPlayerTwo"/></Skill>"#;
        let arc_cible = r#"<Skill enabled="true"><Gem nameSpec="Arc" skillId="ArcPlayer" level="20"/><Gem nameSpec="Zenith II" skillId="SupportZenithPlayerTwo"/><Gem nameSpec="Lightning Penetration" skillId="SupportLightningPenetrationPlayer"/></Skill>"#;
        let orb = r#"<Skill enabled="true"><Gem nameSpec="Orb of Storms" skillId="OrbOfStormsPlayer" level="20"/></Skill>"#;
        let a = build("Sorceress", arc, "", "1,2,3");
        let c = build("Sorceress", &format!("{arc_cible}{orb}"), "", "2,3,4,5");
        let e = ecart(&a, &c);
        assert_eq!(e.competences_manquantes, vec!["Orb of Storms"]);
        assert_eq!(
            e.supports_manquants,
            vec![SupportManquant {
                competence: "Arc".into(),
                support: "Lightning Penetration".into()
            }]
        );
        assert_eq!((e.passifs_a_prendre, e.passifs_a_retirer), (2, 1));
        assert_eq!((e.passifs_actuels, e.passifs_cible), (3, 4));
        assert!(!e.autre_classe);
        assert!(ecart(&a, &build("Witch", "", "", "")).autre_classe);
    }

    #[test]
    fn progression_de_l_objet_copie() {
        let a = build(
            "Sorceress",
            "",
            &(item(1, CASQUE_ACTUEL) + &set(&[("Helmet", 1)])),
            "",
        );
        let c = build(
            "Sorceress",
            "",
            &(item(1, CASQUE_CIBLE) + &set(&[("Helmet", 1)])),
            "",
        );
        let copie = |l: &[&str]| l.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let p = progression(
            Some(&a),
            &c,
            "Helmets",
            "RARE",
            Some("New Crown"),
            "Voltaic Helm",
            &copie(&[
                "+78 to maximum Life",
                "+31% to Fire Resistance",
                "+20% to Cold Resistance",
            ]),
        )
        .unwrap();
        assert_eq!(p.emplacement, "Helmet");
        assert_eq!((p.atteints, p.total), (2, 4));
        assert_eq!(p.mods_manquants, vec!["+35% to Lightning Resistance"]);
        assert_eq!(p.mods_plus_faibles.len(), 1);
        // Le casque porté n'atteint que la résistance au feu.
        assert_eq!(p.actuel_atteints, Some(1));
        // Pas d'objet cible pour cet emplacement, ou classe non suivie.
        assert!(progression(Some(&a), &c, "Gloves", "RARE", None, "X", &[]).is_none());
        assert!(progression(Some(&a), &c, "Life Flasks", "MAGIC", None, "X", &[]).is_none());
    }

    #[test]
    fn progression_vers_un_unique() {
        let tete =
            "Rarity: UNIQUE\nCrown of Eyes\nHubris Circlet\nImplicits: 0\n+50 to maximum Life";
        let c = build(
            "Sorceress",
            "",
            &(item(1, tete) + &set(&[("Helmet", 1)])),
            "",
        );
        let p = progression(
            None,
            &c,
            "Helmets",
            "UNIQUE",
            Some("Crown of Eyes"),
            "Hubris Circlet",
            &[],
        )
        .unwrap();
        assert_eq!((p.statut, p.atteints, p.total), (Statut::Atteint, 1, 1));
        assert_eq!(p.actuel_atteints, None);
        let p = progression(
            None,
            &c,
            "Helmets",
            "RARE",
            Some("Doom Visor"),
            "Hubris Circlet",
            &[],
        )
        .unwrap();
        assert_eq!((p.statut, p.atteints), (Statut::AutreObjet, 0));
    }

    #[test]
    fn objet_cible_pour_le_trade() {
        let texte = "Rarity: RARE\nDoom Visor\nVoltaic Helm\nImplicits: 1\n+20 to maximum Mana\n{enchant}10% increased Life\n{crafted}+80 to maximum Life\n{range:1}+(30-34)% to Fire Resistance";
        let c = build(
            "Sorceress",
            "",
            &(item(1, texte) + &set(&[("Helmet", 1)])),
            "",
        );
        let o = objet_cible(&c, "Helmet").unwrap();
        assert_eq!(o.unique, None);
        assert_eq!(o.base.as_deref(), Some("Voltaic Helm"));
        assert_eq!(
            o.lignes,
            vec!["+80 to maximum Life", "+34% to Fire Resistance"]
        );
        let u = "Rarity: UNIQUE\nCrown of Eyes\nHubris Circlet\nImplicits: 0\n+50 to maximum Life";
        let c = build("Sorceress", "", &(item(1, u) + &set(&[("Helmet", 1)])), "");
        assert_eq!(
            objet_cible(&c, "Helmet").unwrap().unique.as_deref(),
            Some("Crown of Eyes")
        );
        assert!(objet_cible(&c, "Gloves").is_none());
    }

    #[test]
    fn emplacements_poe1() {
        assert_eq!(emplacements("Rings").unwrap(), vec!["Ring 1", "Ring 2"]);
        assert_eq!(
            emplacements("Thrusting One Hand Swords").unwrap(),
            vec!["Weapon 1", "Weapon 2"]
        );
        assert_eq!(emplacements("Two Hand Axes").unwrap(), vec!["Weapon 1"]);
        assert!(emplacements("Life Flasks").is_none());
    }

    #[test]
    fn chiffres_exportes() {
        let mut s = BTreeMap::new();
        for (k, v) in [
            ("FullDPS", 0.0),
            ("CombinedDPS", 12000.0),
            ("Life", 4200.0),
            ("FireResist", 75.0),
            ("ChaosResist", -20.0),
        ] {
            s.insert(k.to_string(), v);
        }
        let c = chiffres_pob(&s);
        assert_eq!(c.dps, Some(12000.0));
        assert_eq!(c.vie, Some(4200.0));
        assert_eq!(c.ehp, None);
        assert_eq!(
            c.resistances,
            vec![(TypeDegats::Feu, 75.0), (TypeDegats::Chaos, -20.0)]
        );
        for t in ["Physical", "Fire", "Cold", "Lightning", "Chaos"] {
            s.insert(format!("{t}MaximumHitTaken"), 5000.0);
        }
        assert_eq!(chiffres_pob(&s).ehp, Some(5000.0));
    }
}
