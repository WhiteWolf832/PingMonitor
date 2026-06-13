// Module "route_view" — vue Route (= route_view.py)
//
// On lance mtr dans un thread de fond et on récupère le résultat sur l'UI via
// un channel jetable (un seul message). Même principe que le moteur de ping.

use std::cell::Cell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

use crate::route::{Hop, run_route};

// (titre, largeur, alignement) — 0.0 gauche, 0.5 centre, 1.0 droite.
const HEADER: &[(&str, i32, f64)] = &[
    ("#", 36, 1.0),
    ("Hôte", 220, 0.0),
    ("Perte", 70, 1.0),
    ("Envoyés", 80, 1.0),
    ("Dernier", 80, 1.0),
    ("Moy", 80, 1.0),
    ("Min", 70, 1.0),
    ("Max", 70, 1.0),
];

pub struct RouteView {
    pub root: gtk::Box,
}

impl RouteView {
    pub fn new(hosts: Vec<(String, String)>) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
        root.set_margin_top(12);
        root.set_margin_bottom(12);
        root.set_margin_start(12);
        root.set_margin_end(12);

        // --- contrôles : destination + bouton + spinner + statut ---
        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        controls.append(&gtk::Label::new(Some("Destination :")));

        let strings: Vec<String> = hosts.iter().map(|(n, a)| format!("{n} ({a})")).collect();
        let sref: Vec<&str> = strings.iter().map(|s| s.as_str()).collect();
        let combo = gtk::DropDown::from_strings(&sref);
        controls.append(&combo);

        let run_btn = gtk::Button::with_label("Lancer le traceroute");
        run_btn.add_css_class("suggested-action");
        controls.append(&run_btn);

        let spinner = gtk::Spinner::new();
        controls.append(&spinner);

        let status = gtk::Label::new(Some(""));
        status.set_xalign(0.0);
        status.set_hexpand(true);
        status.add_css_class("dim-label");
        controls.append(&status);
        root.append(&controls);

        // --- en-tête du tableau de sauts ---
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        header.add_css_class("dim-label");
        for (title, width, align) in HEADER {
            header.append(&cell(title, *width, *align, None));
        }
        root.append(&header);

        // --- lignes (dans un défilement) ---
        let rows_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let scroller = gtk::ScrolledWindow::builder()
            .child(&rows_box)
            .vexpand(true)
            .build();
        root.append(&scroller);

        // Drapeau "en cours" partagé (Rc<Cell<bool>> = mutation simple, sans RefCell).
        let running = Rc::new(Cell::new(false));
        let hosts_rc = Rc::new(hosts);

        run_btn.connect_clicked(glib::clone!(
            #[strong]
            running,
            #[strong]
            hosts_rc,
            #[weak]
            combo,
            #[weak]
            run_btn,
            #[weak]
            spinner,
            #[weak]
            status,
            #[weak]
            rows_box,
            move |_| {
                if running.get() || hosts_rc.is_empty() {
                    return;
                }
                let idx = combo.selected() as usize;
                let Some((_, addr)) = hosts_rc.get(idx) else {
                    return;
                };
                let addr = addr.clone();

                running.set(true);
                run_btn.set_sensitive(false);
                spinner.start();
                status.set_text(&format!("Analyse du chemin vers {addr}…"));
                clear_children(&rows_box);

                // Thread de fond : exécute mtr (bloquant) et renvoie le résultat.
                let (tx, rx) = async_channel::bounded::<Result<Vec<Hop>, String>>(1);
                std::thread::spawn(move || {
                    let _ = tx.send_blocking(run_route(&addr, 3));
                });

                // Réception sur l'UI : reconstruit les lignes quand mtr a fini.
                glib::spawn_future_local(glib::clone!(
                    #[strong]
                    running,
                    #[weak]
                    run_btn,
                    #[weak]
                    spinner,
                    #[weak]
                    status,
                    #[weak]
                    rows_box,
                    async move {
                        if let Ok(result) = rx.recv().await {
                            running.set(false);
                            run_btn.set_sensitive(true);
                            spinner.stop();
                            match result {
                                Err(e) => status.set_text(&format!("Erreur : {e}")),
                                Ok(hops) => {
                                    status.set_text(&format!("{} saut(s)", hops.len()));
                                    for hop in &hops {
                                        rows_box.append(&hop_row(hop));
                                    }
                                }
                            }
                        }
                    }
                ));
            }
        ));

        Self { root }
    }
}

/// Une cellule de largeur fixe contenant un label aligné.
fn cell(text: &str, width: i32, xalign: f64, css: Option<&str>) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    b.set_size_request(width, -1);
    let l = gtk::Label::new(Some(text));
    l.set_hexpand(true);
    l.set_halign(if xalign == 0.0 {
        gtk::Align::Start
    } else if xalign == 0.5 {
        gtk::Align::Center
    } else {
        gtk::Align::End
    });
    if let Some(c) = css {
        l.add_css_class(c);
    }
    b.append(&l);
    b
}

fn clear_children(b: &gtk::Box) {
    while let Some(child) = b.first_child() {
        b.remove(&child);
    }
}

fn hop_row(hop: &Hop) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let host_label = if hop.host == "???" || hop.host.is_empty() {
        "* (pas de réponse)".to_string()
    } else {
        hop.host.clone()
    };
    let loss_css = if hop.loss > 0.0 { Some("error") } else { None };
    row.append(&cell(&hop.idx.to_string(), 36, 1.0, None));
    row.append(&cell(&host_label, 220, 0.0, Some("heading")));
    row.append(&cell(&format!("{:.0}%", hop.loss), 70, 1.0, loss_css));
    row.append(&cell(&hop.sent.to_string(), 80, 1.0, None));
    row.append(&cell(&format!("{:.1}", hop.last), 80, 1.0, None));
    row.append(&cell(&format!("{:.1}", hop.avg), 80, 1.0, None));
    row.append(&cell(&format!("{:.1}", hop.best), 70, 1.0, None));
    row.append(&cell(&format!("{:.1}", hop.worst), 70, 1.0, None));
    row
}
