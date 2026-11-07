//! Atlas Ledger : suivi des sessions de farm à partir de `Client.txt`.
//!
//! - [`journal`] lit les lignes du journal du jeu (génération et entrée de
//!   zone, horodatage) ;
//! - [`zones`] classe les zones (map, hideout, ville…) d'après leur
//!   identifiant interne ;
//! - [`session`] découpe une session en parties de map et répartit le temps.
//!
//! Comme le reste du crate, ce module ne fait aucune entrée-sortie : c'est
//! l'application qui lit le fichier et lui passe les lignes. La valeur du
//! butin n'est pas couverte : l'API des réserves n'est pas ouverte pour
//! PoE 2 (voir `docs/architecture/api-ggg.md`).

pub mod journal;
pub mod session;
pub mod zones;

pub use journal::{EvenementJournal, Horodatage, LigneJournal, analyser_ligne};
pub use session::{Bilan, PartieMap, Repartition, SuiviSession};
pub use zones::{TypeZone, type_zone};
