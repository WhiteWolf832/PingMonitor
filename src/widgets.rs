// Module "widgets" — petits dessins Cairo (= widgets.py)
//
// Chaque widget = un gtk::DrawingArea + une fonction de dessin. L'état (valeurs,
// score, statut) est rangé dans un Rc<RefCell> que la closure de dessin lit, et
// qu'un setter modifie avant d'appeler queue_draw() (= redessine-toi).

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{cairo, glib};

use crate::model::Status;

// Couleurs de qualité (pire → meilleur), identiques à la version Python.
pub const QUALITY_COLORS: [(f64, f64, f64); 5] = [
    (0.90, 0.25, 0.22), // rouge
    (0.95, 0.55, 0.18), // orange
    (0.92, 0.82, 0.25), // jaune
    (0.55, 0.80, 0.30), // vert clair
    (0.30, 0.78, 0.35), // vert
];

fn status_color(s: Status) -> (f64, f64, f64) {
    match s {
        Status::Online => (0.30, 0.78, 0.35),
        Status::Degraded => (0.95, 0.65, 0.18),
        Status::Offline => (0.90, 0.25, 0.22),
        Status::Unknown => (0.45, 0.48, 0.52),
    }
}

/// Pastille de couleur indiquant l'état.
pub struct StatusDot {
    pub area: gtk::DrawingArea,
    status: Rc<RefCell<Status>>,
}

impl StatusDot {
    pub fn new() -> Self {
        let area = gtk::DrawingArea::new();
        area.set_content_width(14);
        area.set_content_height(14);
        let status = Rc::new(RefCell::new(Status::Unknown));
        // La closure de dessin co-possède `status` (clone du Rc).
        area.set_draw_func(glib::clone!(
            #[strong]
            status,
            move |_area, cr, w, h| {
                let (r, g, b) = status_color(*status.borrow());
                let radius = (w.min(h) as f64) / 2.0 - 2.0;
                cr.set_source_rgba(r, g, b, 1.0);
                cr.arc(
                    w as f64 / 2.0,
                    h as f64 / 2.0,
                    radius,
                    0.0,
                    std::f64::consts::TAU,
                );
                let _ = cr.fill(); // .fill() renvoie un Result ; on ignore l'erreur
            }
        ));
        Self { area, status }
    }

    pub fn set_status(&self, s: Status) {
        *self.status.borrow_mut() = s;
        self.area.queue_draw();
    }
}

/// Mini-courbe de l'historique récent des latences.
pub struct Sparkline {
    pub area: gtk::DrawingArea,
    values: Rc<RefCell<Vec<Option<f64>>>>,
}

impl Sparkline {
    pub fn new() -> Self {
        let area = gtk::DrawingArea::new();
        area.set_content_width(110);
        area.set_content_height(24);
        let values: Rc<RefCell<Vec<Option<f64>>>> = Rc::new(RefCell::new(Vec::new()));
        area.set_draw_func(glib::clone!(
            #[strong]
            values,
            move |_area, cr, w, h| {
                draw_sparkline(cr, w, h, &values.borrow());
            }
        ));
        Self { area, values }
    }

    pub fn set_values(&self, v: Vec<Option<f64>>) {
        *self.values.borrow_mut() = v;
        self.area.queue_draw();
    }
}

fn draw_sparkline(cr: &cairo::Context, width: i32, height: i32, values: &[Option<f64>]) {
    let vals: Vec<f64> = values.iter().filter_map(|v| *v).collect();
    if values.len() < 2 || vals.is_empty() {
        return;
    }
    // min/max via fold (pas de min() direct sur les f64 à cause des NaN).
    let lo = vals.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let span = (hi - lo).max(1e-6);
    let n = values.len();
    let pad = 2.0;
    let w = width as f64;
    let usable_h = height as f64 - 2.0 * pad;
    // Closures locales pour convertir index→x et valeur→y.
    let x = |i: usize| i as f64 * (w - 1.0) / (n as f64 - 1.0);
    let y = |v: f64| pad + usable_h * (1.0 - (v - lo) / span);

    // La courbe verte. On coupe le tracé sur les trous (pertes).
    cr.set_line_width(1.4);
    cr.set_source_rgba(0.45, 0.78, 0.55, 0.95);
    let mut started = false;
    for (i, v) in values.iter().enumerate() {
        match v {
            Some(val) => {
                let (px, py) = (x(i), y(*val));
                if !started {
                    cr.move_to(px, py);
                    started = true;
                } else {
                    cr.line_to(px, py);
                }
            }
            None => started = false, // perte → on lève le crayon
        }
    }
    let _ = cr.stroke();

    // Petits traits rouges aux pertes.
    cr.set_source_rgba(0.90, 0.25, 0.22, 0.9);
    for (i, v) in values.iter().enumerate() {
        if v.is_none() {
            cr.rectangle(x(i) - 0.5, pad, 1.5, usable_h);
            let _ = cr.fill();
        }
    }
}

/// Cinq carrés colorés selon le score 0..5 (chacun garde sa couleur de position).
pub struct QualityBars {
    pub area: gtk::DrawingArea,
    score: Rc<RefCell<i32>>,
}

impl QualityBars {
    pub fn new() -> Self {
        let area = gtk::DrawingArea::new();
        area.set_content_width(78);
        area.set_content_height(16);
        let score = Rc::new(RefCell::new(0));
        area.set_draw_func(glib::clone!(
            #[strong]
            score,
            move |_area, cr, w, h| {
                draw_quality(cr, w, h, *score.borrow());
            }
        ));
        Self { area, score }
    }

    pub fn set_score(&self, s: i32) {
        *self.score.borrow_mut() = s.clamp(0, 5);
        self.area.queue_draw();
    }
}

fn draw_quality(cr: &cairo::Context, width: i32, height: i32, score: i32) {
    let n = 5usize;
    let gap = 3.0;
    let w = width as f64;
    let h = height as f64;
    let size = ((w - (n as f64 - 1.0) * gap) / n as f64).min(h);
    let y = (h - size) / 2.0;
    for i in 0..n {
        let xpos = i as f64 * (size + gap);
        if (i as i32) < score {
            let (r, g, b) = QUALITY_COLORS[i];
            cr.set_source_rgba(r, g, b, 1.0);
        } else {
            cr.set_source_rgba(0.30, 0.33, 0.38, 1.0); // carré éteint (gris)
        }
        cr.rectangle(xpos, y, size, size);
        let _ = cr.fill();
    }
}
