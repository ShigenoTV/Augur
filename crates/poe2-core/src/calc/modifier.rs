//! Modificateurs typés.

use serde::{Deserialize, Serialize};

/// La nature d'un modificateur, dans l'ordre d'application du moteur :
/// `base` et `added` s'additionnent, puis `increased` (additifs entre eux,
/// une valeur négative vaut « reduced »), puis chaque `more` multiplie
/// (une valeur négative vaut « less »). `override` remplace le résultat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TypeModificateur {
    Base,
    Added,
    Increased,
    More,
    Override,
}

impl TypeModificateur {
    pub fn libelle(self) -> &'static str {
        match self {
            TypeModificateur::Base => "base",
            TypeModificateur::Added => "added",
            TypeModificateur::Increased => "increased",
            TypeModificateur::More => "more",
            TypeModificateur::Override => "override",
        }
    }
}

/// Un modificateur appliqué à une stat, avec sa provenance.
///
/// La source est libre (« Passif : Inferno », « Objet : Bague, mod 2 »…) :
/// c'est ce qui permet au Theorycrafter de retrouver une erreur dans la trace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Modificateur {
    pub stat: String,
    #[serde(rename = "type")]
    pub kind: TypeModificateur,
    pub valeur: f64,
    #[serde(default = "source_inconnue")]
    pub source: String,
}

fn source_inconnue() -> String {
    "source non précisée".to_string()
}

impl Modificateur {
    pub fn new(
        stat: impl Into<String>,
        kind: TypeModificateur,
        valeur: f64,
        source: impl Into<String>,
    ) -> Self {
        Self {
            stat: stat.into(),
            kind,
            valeur,
            source: source.into(),
        }
    }
}
