//! Extraction des valeurs numériques d'une ligne de mod.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ValeursMod {
    pub texte: String,
    pub modele: String,
    pub valeurs: Vec<f64>,
    pub plages: Vec<Option<(f64, f64)>>,
}

/// Lit un nombre décimal non signé à partir de `i`. Renvoie (valeur, fin).
fn nombre(c: &[char], i: usize) -> Option<(f64, usize)> {
    let mut j = i;
    while j < c.len() && c[j].is_ascii_digit() {
        j += 1;
    }
    if j == i {
        return None;
    }
    if j + 1 < c.len() && c[j] == '.' && c[j + 1].is_ascii_digit() {
        j += 1;
        while j < c.len() && c[j].is_ascii_digit() {
            j += 1;
        }
    }
    let s: String = c[i..j].iter().collect();
    s.parse().ok().map(|v| (v, j))
}

/// Lit une plage avancée `(4-9)` ou `(-9--4)` à partir de `i`.
fn plage(c: &[char], i: usize) -> Option<((f64, f64), usize)> {
    if c.get(i) != Some(&'(') {
        return None;
    }
    let mut j = i + 1;
    let neg_a = c.get(j) == Some(&'-');
    if neg_a {
        j += 1;
    }
    let (a, k) = nombre(c, j)?;
    if c.get(k) != Some(&'-') {
        return None;
    }
    let mut j = k + 1;
    let neg_b = c.get(j) == Some(&'-');
    if neg_b {
        j += 1;
    }
    let (b, k) = nombre(c, j)?;
    if c.get(k) != Some(&')') {
        return None;
    }
    Some((
        (if neg_a { -a } else { a }, if neg_b { -b } else { b }),
        k + 1,
    ))
}

/// Extrait les valeurs d'une ligne, remplace chaque nombre par `#` dans le
/// modèle et retire les plages avancées du texte.
///
/// Le signe `+` reste dans le modèle (`+#% to Fire Resistance`), comme dans
/// les descriptions de stats du jeu ; un `-` collé au nombre est porté par la
/// valeur.
pub fn extraire_valeurs(ligne: &str) -> ValeursMod {
    let c: Vec<char> = ligne.chars().collect();
    let mut texte = String::with_capacity(ligne.len());
    let mut modele = String::with_capacity(ligne.len());
    let mut valeurs = Vec::new();
    let mut plages = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let negatif = c[i] == '-'
            && c.get(i + 1).is_some_and(|d| d.is_ascii_digit())
            && (i == 0 || !c[i - 1].is_alphanumeric());
        let debut = if negatif { i + 1 } else { i };
        let precedent_alnum = debut > 0 && c[debut - 1].is_alphanumeric();
        if c[debut].is_ascii_digit()
            && !precedent_alnum
            && let Some((v, fin)) = nombre(&c, debut)
        {
            let v = if negatif { -v } else { v };
            texte.extend(&c[i..fin]);
            modele.push('#');
            valeurs.push(v);
            match plage(&c, fin) {
                Some((p, apres)) => {
                    plages.push(Some(p));
                    i = apres;
                }
                None => {
                    plages.push(None);
                    i = fin;
                }
            }
            continue;
        }
        texte.push(c[i]);
        modele.push(c[i]);
        i += 1;
    }
    ValeursMod {
        texte,
        modele,
        valeurs,
        plages,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple() {
        let v = extraire_valeurs("+38% to Fire Resistance");
        assert_eq!(v.modele, "+#% to Fire Resistance");
        assert_eq!(v.valeurs, vec![38.0]);
    }

    #[test]
    fn avance_avec_plages() {
        let v = extraire_valeurs("Adds 5(4-6) to 9(8-10) Fire Damage");
        assert_eq!(v.texte, "Adds 5 to 9 Fire Damage");
        assert_eq!(v.modele, "Adds # to # Fire Damage");
        assert_eq!(v.valeurs, vec![5.0, 9.0]);
        assert_eq!(v.plages, vec![Some((4.0, 6.0)), Some((8.0, 10.0))]);
    }

    #[test]
    fn negatif_et_decimal() {
        let v = extraire_valeurs("-10% to Cold Resistance, 1.5 metres");
        assert_eq!(v.valeurs, vec![-10.0, 1.5]);
        assert_eq!(v.modele, "#% to Cold Resistance, # metres");
    }

    #[test]
    fn chiffre_dans_un_mot_ignore() {
        let v = extraire_valeurs("Grants Level 20 Spell2Echo");
        assert_eq!(v.valeurs, vec![20.0]);
    }
}
