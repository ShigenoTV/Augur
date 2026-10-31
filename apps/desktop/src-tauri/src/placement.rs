//! Placement du panneau de l'overlay près du curseur, dans l'écran courant.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Décalage entre le curseur et le panneau, pour ne pas masquer le tooltip du jeu.
pub const CURSOR_GAP: f64 = 24.0;

/// Place le panneau à droite du curseur, ou à gauche s'il déborderait de
/// l'écran, aligné verticalement sur le curseur puis contenu dans l'écran.
pub fn place_panel(cursor: (f64, f64), panel: (f64, f64), screen: Rect) -> (f64, f64) {
    let (cx, cy) = cursor;
    let (w, h) = panel;
    let right_edge = screen.x + screen.width;
    let bottom_edge = screen.y + screen.height;

    let x = if cx + CURSOR_GAP + w <= right_edge {
        cx + CURSOR_GAP
    } else {
        (cx - CURSOR_GAP - w).max(screen.x)
    };
    let y = (cy - 48.0).min(bottom_edge - h).max(screen.y);
    (x, y)
}

/// Distance au-delà de laquelle l'overlay se ferme quand le curseur s'éloigne.
pub const DISMISS_DISTANCE: f64 = 360.0;

/// Vrai si le curseur s'est assez éloigné du point d'ancrage, hors du panneau.
pub fn should_dismiss(anchor: (f64, f64), cursor: (f64, f64), panel: Rect) -> bool {
    let inside = cursor.0 >= panel.x
        && cursor.0 <= panel.x + panel.width
        && cursor.1 >= panel.y
        && cursor.1 <= panel.y + panel.height;
    if inside {
        return false;
    }
    let (dx, dy) = (cursor.0 - anchor.0, cursor.1 - anchor.1);
    (dx * dx + dy * dy).sqrt() > DISMISS_DISTANCE
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
    };

    #[test]
    fn a_droite_du_curseur_par_defaut() {
        assert_eq!(
            place_panel((500.0, 400.0), (380.0, 600.0), SCREEN),
            (524.0, 352.0)
        );
    }

    #[test]
    fn bascule_a_gauche_pres_du_bord_droit() {
        assert_eq!(
            place_panel((1800.0, 400.0), (380.0, 600.0), SCREEN),
            (1396.0, 352.0)
        );
    }

    #[test]
    fn reste_dans_l_ecran_verticalement() {
        assert_eq!(
            place_panel((500.0, 1000.0), (380.0, 600.0), SCREEN).1,
            480.0
        );
        assert_eq!(place_panel((500.0, 10.0), (380.0, 600.0), SCREEN).1, 0.0);
    }

    #[test]
    fn respecte_un_second_ecran() {
        let second = Rect {
            x: 1920.0,
            y: 0.0,
            width: 2560.0,
            height: 1440.0,
        };
        assert_eq!(
            place_panel((4400.0, 700.0), (380.0, 600.0), second).0,
            3996.0
        );
    }

    #[test]
    fn ferme_quand_le_curseur_s_eloigne() {
        let panel = Rect {
            x: 524.0,
            y: 352.0,
            width: 380.0,
            height: 600.0,
        };
        assert!(!should_dismiss((500.0, 400.0), (520.0, 420.0), panel));
        assert!(
            !should_dismiss((500.0, 400.0), (880.0, 900.0), panel),
            "dans le panneau"
        );
        assert!(should_dismiss((500.0, 400.0), (100.0, 900.0), panel));
    }
}
