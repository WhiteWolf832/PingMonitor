// Module "graph_view" — graphique de latence navigable (= graph_view.py)
//
// Concepts Rust/GTK introduits :
//   - les contrôleurs d'événements (molette, glisser) branchés sur un DrawingArea
//   - le partage d'un état (GraphState) entre la closure de dessin ET les
//     contrôleurs, via Rc<RefCell>
//   - glib::DateTime pour formater les horodatages en heure locale
//   - les closures locales |x| ... dans le dessin (conversions temps→pixel)

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{cairo, glib};

use crate::model::now_ts;
use crate::storage::Storage;

// (libellé, durée en secondes). Libellés en unités, non traduits.
const RANGES: &[(&str, f64)] = &[
    ("5m", 300.0),
    ("30m", 1800.0),
    ("1h", 3600.0),
    ("2h", 7200.0),
    ("4h", 14400.0),
    ("8h", 28800.0),
    ("24h", 86400.0),
    ("7d", 604800.0),
];
const MAX_PAST: f64 = 30.0 * 24.0 * 3600.0;

/// État interne du graphique, partagé entre dessin et contrôleurs.
struct GraphState {
    hosts: Vec<(String, String)>, // (nom, adresse)
    address: Option<String>,
    range: f64,
    end: Option<f64>, // None = mode direct (suit "maintenant")
    last_plot_w: f64,
    drag_anchor: f64,
}

pub struct GraphView {
    pub root: gtk::Box,
    pub area: gtk::DrawingArea,
    state: Rc<RefCell<GraphState>>,
}

impl GraphView {
    pub fn new(storage: Rc<Storage>, hosts: Vec<(String, String)>) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);

        // Adresse initiale = 1er hôte (le graphique n'est pas vide au lancement).
        let initial_addr = hosts.first().map(|(_, a)| a.clone());

        let state = Rc::new(RefCell::new(GraphState {
            hosts: hosts.clone(),
            address: initial_addr,
            range: RANGES[0].1,
            end: None,
            last_plot_w: 1.0,
            drag_anchor: 0.0,
        }));

        // --- barre supérieure : fenêtre visible + Direct + plages ---
        // (L'hôte affiché se choisit en cliquant une ligne du tableau, plus de
        // menu déroulant ici.)
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        bar.set_margin_top(8);
        bar.set_margin_bottom(4);
        bar.set_margin_start(8);
        bar.set_margin_end(8);

        let info = gtk::Label::new(Some(""));
        info.set_xalign(0.0);
        info.set_hexpand(true);
        info.add_css_class("dim-label");
        // Ellipse : le label peut rétrécir au lieu d'imposer sa largeur de texte
        // (sinon il pousse la fenêtre à s'élargir quand le texte change).
        info.set_ellipsize(gtk::pango::EllipsizeMode::End);

        let live_btn = gtk::Button::with_label("⟲ Direct");
        live_btn.add_css_class("flat");
        // On garde le bouton dans la disposition en permanence (espace réservé) :
        // transparent + inactif quand on est en direct. Ainsi la largeur de la
        // barre — donc la largeur minimale de la fenêtre — ne change jamais.
        live_btn.set_opacity(0.0);
        live_btn.set_sensitive(false);

        // Plages : un menu déroulant (plus compact que 8 boutons → fenêtre
        // beaucoup plus étroite possible).
        let range_labels: Vec<&str> = RANGES.iter().map(|(l, _)| *l).collect();
        let range_combo = gtk::DropDown::from_strings(&range_labels);
        range_combo.set_selected(0);

        bar.append(&info);
        bar.append(&live_btn);
        bar.append(&range_combo);
        root.append(&bar);

        // --- la zone de dessin ---
        let area = gtk::DrawingArea::new();
        area.set_vexpand(true);
        area.set_hexpand(true);
        area.set_content_height(180);
        area.set_cursor_from_name(Some("grab"));
        root.append(&area);

        // Fonction de dessin : lit l'état + interroge la base à chaque rendu.
        area.set_draw_func(glib::clone!(
            #[strong]
            state,
            #[strong]
            storage,
            #[weak]
            info,
            move |_a, cr, w, h| {
                draw_graph(cr, w, h, &state, &storage, &info);
            }
        ));

        // --- sélecteur de plage (menu déroulant) ---
        range_combo.connect_selected_notify(glib::clone!(
            #[strong]
            state,
            #[weak]
            area,
            #[weak]
            live_btn,
            move |dd| {
                let idx = dd.selected() as usize;
                let secs = RANGES.get(idx).map(|(_, s)| *s).unwrap_or(RANGES[0].1);
                let end_opt = {
                    let mut st = state.borrow_mut();
                    st.range = secs;
                    st.end
                };
                match end_opt {
                    Some(e) => pan_to(&state, &area, &live_btn, e),
                    None => area.queue_draw(),
                }
            }
        ));

        // --- bouton Direct ---
        live_btn.connect_clicked(glib::clone!(
            #[strong]
            state,
            #[weak]
            area,
            move |btn| {
                state.borrow_mut().end = None;
                btn.set_opacity(0.0);
                btn.set_sensitive(false);
                area.queue_draw();
            }
        ));

        // --- molette : avancer/reculer dans le temps ---
        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
        scroll.connect_scroll(glib::clone!(
            #[strong]
            state,
            #[weak]
            area,
            #[weak]
            live_btn,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_c, dx, dy| {
                let (range, eff_end) = {
                    let st = state.borrow();
                    (st.range, st.end.unwrap_or_else(now_ts))
                };
                pan_to(&state, &area, &live_btn, eff_end + (dx + dy) * 0.2 * range);
                glib::Propagation::Stop
            }
        ));
        area.add_controller(scroll);

        // --- glisser : panoramique horizontal ---
        let drag = gtk::GestureDrag::new();
        drag.connect_drag_begin(glib::clone!(
            #[strong]
            state,
            #[weak]
            area,
            move |_g, _x, _y| {
                let anchor = {
                    let st = state.borrow();
                    st.end.unwrap_or_else(now_ts)
                };
                state.borrow_mut().drag_anchor = anchor;
                area.set_cursor_from_name(Some("grabbing"));
            }
        ));
        drag.connect_drag_update(glib::clone!(
            #[strong]
            state,
            #[weak]
            area,
            #[weak]
            live_btn,
            move |_g, ox, _oy| {
                let (range, anchor, last_w) = {
                    let st = state.borrow();
                    (st.range, st.drag_anchor, st.last_plot_w)
                };
                let delta = -(ox / last_w) * range;
                pan_to(&state, &area, &live_btn, anchor + delta);
            }
        ));
        drag.connect_drag_end(glib::clone!(
            #[weak]
            area,
            move |_g, _ox, _oy| {
                area.set_cursor_from_name(Some("grab"));
            }
        ));
        area.add_controller(drag);

        // La plage initiale (RANGES[0]) est déjà dans l'état ; la zone se dessine
        // automatiquement à son premier affichage.

        Self { root, area, state }
    }

    /// Redessine si on est en mode direct (ne dérange pas une exploration en cours).
    pub fn refresh(&self) {
        if self.state.borrow().end.is_none() {
            self.area.queue_draw();
        }
    }

    /// Sélectionne l'hôte d'adresse `address` dans le graphique (depuis le tableau).
    /// On met à jour l'état puis on redessine.
    pub fn select_host(&self, address: &str) {
        // On vérifie d'abord que l'adresse est connue (sinon on ne change rien).
        let known = self
            .state
            .borrow()
            .hosts
            .iter()
            .any(|(_, a)| a == address);
        if known {
            self.state.borrow_mut().address = Some(address.to_string());
            self.area.queue_draw();
        }
    }
}

/// Recale le bord droit de la fenêtre visible et bascule live/pause.
fn pan_to(
    state: &Rc<RefCell<GraphState>>,
    area: &gtk::DrawingArea,
    live_btn: &gtk::Button,
    new_end: f64,
) {
    let now = now_ts();
    let is_live;
    {
        let mut st = state.borrow_mut();
        let floor = now - MAX_PAST + st.range;
        let clamped = new_end.min(now).max(floor);
        st.end = if clamped >= now - 0.5 {
            None
        } else {
            Some(clamped)
        };
        is_live = st.end.is_none();
    }
    // Pas de set_visible : on module l'opacité/l'activité pour ne pas modifier
    // la disposition (et donc ne pas faire grandir la fenêtre).
    live_btn.set_opacity(if is_live { 0.0 } else { 1.0 });
    live_btn.set_sensitive(!is_live);
    area.queue_draw();
}

// ------------------------------------------------------------------ dessin

fn draw_graph(
    cr: &cairo::Context,
    width: i32,
    height: i32,
    state: &Rc<RefCell<GraphState>>,
    storage: &Storage,
    info: &gtk::Label,
) {
    let (ml, mr, mt, mb) = (44.0_f64, 12.0, 12.0, 22.0);
    let plot_w = (width as f64 - ml - mr).max(1.0);
    let plot_h = (height as f64 - mt - mb).max(1.0);

    let (address, range, end_opt) = {
        let mut st = state.borrow_mut();
        st.last_plot_w = plot_w;
        (st.address.clone(), st.range, st.end)
    };
    let end = end_opt.unwrap_or_else(now_ts);
    let since = end - range;

    // Fond du tracé.
    cr.set_source_rgba(0.07, 0.09, 0.11, 1.0);
    cr.rectangle(ml, mt, plot_w, plot_h);
    let _ = cr.fill();

    info.set_text(&window_label(since, end, end_opt.is_some()));

    // `let Some(x) = ... else { return; }` = on sort proprement si pas d'hôte.
    let Some(addr) = address else {
        draw_placeholder(cr, ml, mt, plot_w, plot_h, "Aucun hôte sélectionné");
        return;
    };

    let rows = storage.range(&addr, since, end).unwrap_or_default();
    if rows.is_empty() {
        grid(cr, ml, mt, plot_w, plot_h, 0.0, 50.0, since, end);
        draw_placeholder(cr, ml, mt, plot_w, plot_h, "Aucune donnée sur cette période");
        return;
    }

    let lats: Vec<f64> = rows.iter().filter_map(|r| r.1).collect();
    let hi_raw = lats.iter().copied().fold(0.0_f64, f64::max);
    let hi = if lats.is_empty() { 50.0 } else { (hi_raw * 1.15).max(10.0) };
    let lo = 0.0;

    grid(cr, ml, mt, plot_w, plot_h, lo, hi, since, end);

    // Conversions temps→x et valeur→y (closures locales, comme en Python).
    let x = |ts: f64| ml + plot_w * (ts - since) / (end - since).max(1e-6);
    let y = |v: f64| mt + plot_h * (1.0 - (v - lo) / (hi - lo).max(1e-6));

    // Seuil de trou : 6× la cadence médiane (déduit des écarts réels).
    let mut gap_threshold = f64::INFINITY;
    if rows.len() >= 3 {
        let mut deltas: Vec<f64> = (1..rows.len()).map(|i| rows[i].0 - rows[i - 1].0).collect();
        deltas.sort_by(|a, b| a.partial_cmp(b).unwrap());
        gap_threshold = (deltas[deltas.len() / 2] * 6.0).max(6.0);
    }

    // Découpe en segments : coupé sur les pertes ET sur les trous de données.
    let mut segments: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut cur: Vec<(f64, f64)> = Vec::new();
    let mut prev_ts: Option<f64> = None;
    for &(ts, lat) in &rows {
        if let Some(p) = prev_ts {
            if ts - p > gap_threshold && !cur.is_empty() {
                segments.push(std::mem::take(&mut cur));
            }
        }
        prev_ts = Some(ts);
        match lat {
            None => {
                if !cur.is_empty() {
                    segments.push(std::mem::take(&mut cur));
                }
            }
            Some(v) => cur.push((ts, v)),
        }
    }
    if !cur.is_empty() {
        segments.push(cur);
    }

    // Aire sous la courbe (jaune translucide).
    for seg in &segments {
        if seg.len() < 2 {
            continue;
        }
        cr.move_to(x(seg[0].0), y(0.0));
        for &(ts, lat) in seg {
            cr.line_to(x(ts), y(lat));
        }
        cr.line_to(x(seg[seg.len() - 1].0), y(0.0));
        cr.close_path();
        cr.set_source_rgba(0.62, 0.58, 0.18, 0.30);
        let _ = cr.fill();
    }

    // La ligne de latence (jaune vif).
    cr.set_line_width(1.6);
    cr.set_source_rgba(0.85, 0.78, 0.25, 0.95);
    for seg in &segments {
        let mut started = false;
        for &(ts, lat) in seg {
            let (px, py) = (x(ts), y(lat));
            if !started {
                cr.move_to(px, py);
                started = true;
            } else {
                cr.line_to(px, py);
            }
        }
        let _ = cr.stroke();
    }

    // Marques de perte (verticales rouges).
    cr.set_source_rgba(0.90, 0.25, 0.22, 0.55);
    for &(ts, lat) in &rows {
        if lat.is_none() {
            cr.rectangle(x(ts) - 0.5, mt, 1.0, plot_h);
            let _ = cr.fill();
        }
    }
}

fn window_label(since: f64, end: f64, paused: bool) -> String {
    let span = end - since;
    let fmt = if span > 36.0 * 3600.0 {
        "%d/%m %H:%M"
    } else {
        "%H:%M"
    };
    let suffix = if paused { "  ⏸" } else { "" };
    format!("{} → {}{}", fmt_time(since, fmt), fmt_time(end, fmt), suffix)
}

/// Formate un horodatage epoch en heure locale via glib::DateTime.
fn fmt_time(ts: f64, fmt: &str) -> String {
    glib::DateTime::from_unix_local(ts as i64)
        .and_then(|dt| dt.format(fmt))
        .map(|g| g.to_string())
        .unwrap_or_default()
}

fn grid(cr: &cairo::Context, ml: f64, mt: f64, w: f64, h: f64, lo: f64, hi: f64, since: f64, end: f64) {
    cr.set_line_width(1.0);
    cr.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    cr.set_font_size(10.0);

    // Lignes horizontales + étiquettes Y.
    let steps = 5;
    for i in 0..=steps {
        let frac = i as f64 / steps as f64;
        let yy = mt + h * frac;
        let val = hi - (hi - lo) * frac;
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.07);
        cr.move_to(ml, yy);
        cr.line_to(ml + w, yy);
        let _ = cr.stroke();
        cr.set_source_rgba(0.75, 0.78, 0.82, 0.8);
        cr.move_to(6.0, yy + 3.0);
        let _ = cr.show_text(&format!("{val:.0}"));
    }
    cr.set_source_rgba(0.6, 0.63, 0.67, 0.8);
    cr.move_to(6.0, mt - 2.0);
    let _ = cr.show_text("ms");

    // Lignes verticales + étiquettes X.
    let span = end - since;
    for frac in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let ts = since + span * frac;
        let xx = ml + w * frac;
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.05);
        cr.move_to(xx, mt);
        cr.line_to(xx, mt + h);
        let _ = cr.stroke();
        let label = if span > 36.0 * 3600.0 {
            fmt_time(ts, "%d/%m")
        } else if span > 3.0 * 3600.0 {
            fmt_time(ts, "%Hh")
        } else {
            fmt_time(ts, "%H:%M")
        };
        cr.set_source_rgba(0.75, 0.78, 0.82, 0.8);
        let ext = cr.text_extents(&label).map(|e| e.width()).unwrap_or(0.0);
        let tx = (xx - ext / 2.0).clamp(ml, ml + w - ext);
        cr.move_to(tx, mt + h + 14.0);
        let _ = cr.show_text(&label);
    }
}

fn draw_placeholder(cr: &cairo::Context, ml: f64, mt: f64, w: f64, h: f64, text: &str) {
    cr.set_source_rgba(0.6, 0.63, 0.67, 0.8);
    cr.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    cr.set_font_size(13.0);
    let ext = cr.text_extents(text).map(|e| e.width()).unwrap_or(0.0);
    cr.move_to(ml + (w - ext) / 2.0, mt + h / 2.0);
    let _ = cr.show_text(text);
}
