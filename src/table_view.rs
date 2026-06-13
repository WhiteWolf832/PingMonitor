// Module "table_view" — la vue Tableau (= table_view.py)
//
// Un en-tête de colonnes + une gtk::ListBox (une ligne par hôte). La ListBox
// gère gratuitement la sélection : cliquer une ligne la met en surbrillance et
// déclenche le signal "row-selected" — on s'en sert pour piloter le graphique.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;

use crate::i18n::tr;
use crate::model::Host;
use crate::widgets::{QualityBars, Sparkline, StatusDot};

// (titre source anglais, largeur, alignement xalign, élastique ?) — traduit à
// l'affichage. La colonne "élastique" (l'historique) absorbe la place restante :
// la ligne peut donc rétrécir au lieu de déborder, ce qui évite une fine barre
// de défilement horizontale due au padding interne des GtkListBoxRow.
const COLUMNS: &[(&str, i32, f64, bool)] = &[
    ("", 26, 0.5, false),
    ("Name", 180, 0.0, false),
    ("Address", 150, 0.0, false),
    ("Latency", 90, 1.0, false),
    ("History", 90, 0.5, true),
    ("Loss", 70, 1.0, false),
    ("Jitter", 70, 1.0, false),
    ("Quality", 90, 0.5, false),
];

/// Une ligne du tableau : tous les widgets d'un hôte.
pub struct HostRow {
    dot: StatusDot,
    name: gtk::Label,
    address: gtk::Label,
    latency: gtk::Label,
    spark: Sparkline,
    loss: gtk::Label,
    jitter: gtk::Label,
    quality: QualityBars,
}

impl HostRow {
    /// Rafraîchit tous les widgets de la ligne à partir de l'état de l'hôte.
    pub fn update(&self, host: &Host) {
        self.dot.set_status(host.status);
        self.name.set_text(&host.name);
        self.address.set_text(&host.address);

        self.latency.set_text(&match host.last_latency() {
            Some(ms) => format!("{ms:.1} ms"),
            None => "—".to_string(),
        });

        self.spark.set_values(host.sparkline());
        self.loss.set_text(&format!("{:.0} %", host.loss_pct()));

        self.jitter.set_text(&match host.jitter() {
            Some(j) => format!("{j:.0} ms"),
            None => "—".to_string(),
        });

        self.quality.set_score(host.quality());
        // L'infobulle détaillée s'affiche au survol des carrés de qualité.
        self.quality
            .area
            .set_tooltip_text(Some(&host.quality_tooltip()));
    }
}

/// La vue Tableau complète.
pub struct TableView {
    pub root: gtk::Box,
    listbox: gtk::ListBox,
    // Adresses dans l'ordre d'ajout (= ordre des lignes). Partagé avec le callback
    // de sélection pour retrouver l'hôte cliqué.
    addrs: Rc<RefCell<Vec<String>>>,
}

// Enveloppe un widget dans une cellule alignée. `width` est une largeur fixe,
// sauf si `expand` : la cellule devient élastique (largeur = minimum, puis grandit).
fn cell(widget: &impl IsA<gtk::Widget>, width: i32, xalign: f64, expand: bool) -> gtk::Box {
    let c = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    if expand {
        c.set_hexpand(true); // prend la place restante de la ligne
        c.set_size_request(width, -1); // mais pas en dessous de `width`
    } else {
        c.set_size_request(width, -1); // largeur fixe
    }
    let align = if xalign == 0.0 {
        gtk::Align::Start
    } else if xalign == 0.5 {
        gtk::Align::Center
    } else {
        gtk::Align::End
    };
    widget.set_halign(align);
    widget.set_hexpand(true);
    widget.set_valign(gtk::Align::Center);
    c.append(widget);
    c
}

impl TableView {
    pub fn new() -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.set_margin_top(8);
        root.set_margin_bottom(8);
        root.set_margin_start(8);
        root.set_margin_end(8);

        // En-tête de colonnes (mêmes largeurs que les lignes → colonnes alignées).
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        header.set_margin_start(8);
        header.set_margin_end(8);
        for (title, width, xalign, expand) in COLUMNS {
            let lbl = gtk::Label::new(Some(&tr(title)));
            lbl.add_css_class("dim-label");
            header.append(&cell(&lbl, *width, *xalign, *expand));
        }
        root.append(&header);

        let listbox = gtk::ListBox::new();
        listbox.set_selection_mode(gtk::SelectionMode::Single);
        let scroller = gtk::ScrolledWindow::builder()
            .child(&listbox)
            .vexpand(true)
            .hexpand(true)
            // Jamais de barre horizontale : à largeur réduite, la colonne
            // élastique (History) absorbe le manque de place et la ligne se
            // rogne légèrement, au lieu d'afficher une barre pour quelques
            // pixels (le padding interne des GtkListBoxRow débordait toujours).
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        root.append(&scroller);

        Self {
            root,
            listbox,
            addrs: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Ajoute une ligne pour un hôte et renvoie son HostRow (pour màj ultérieure).
    pub fn add_host(&mut self, address: &str) -> HostRow {
        let dot = StatusDot::new();
        let name = label_cell("", false);
        let address_lbl = label_cell("", true);
        let latency = label_cell("—", false);
        latency.add_css_class("numeric");
        let spark = Sparkline::new();
        let loss = label_cell("—", false);
        loss.add_css_class("numeric");
        let jitter = label_cell("—", false);
        jitter.add_css_class("numeric");
        let quality = QualityBars::new();

        // Une ligne = un Box horizontal de cellules de largeur fixe.
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        row.set_margin_start(8);
        row.set_margin_end(8);
        row.append(&cell(&dot.area, COLUMNS[0].1, COLUMNS[0].2, COLUMNS[0].3));
        row.append(&cell(&name, COLUMNS[1].1, COLUMNS[1].2, COLUMNS[1].3));
        row.append(&cell(&address_lbl, COLUMNS[2].1, COLUMNS[2].2, COLUMNS[2].3));
        row.append(&cell(&latency, COLUMNS[3].1, COLUMNS[3].2, COLUMNS[3].3));
        row.append(&cell(&spark.area, COLUMNS[4].1, COLUMNS[4].2, COLUMNS[4].3));
        row.append(&cell(&loss, COLUMNS[5].1, COLUMNS[5].2, COLUMNS[5].3));
        row.append(&cell(&jitter, COLUMNS[6].1, COLUMNS[6].2, COLUMNS[6].3));
        row.append(&cell(&quality.area, COLUMNS[7].1, COLUMNS[7].2, COLUMNS[7].3));

        self.listbox.append(&row);
        self.addrs.borrow_mut().push(address.to_string());

        HostRow {
            dot,
            name,
            address: address_lbl,
            latency,
            spark,
            loss,
            jitter,
            quality,
        }
    }

    /// Branche un callback appelé avec l'adresse de l'hôte sélectionné au clic.
    pub fn connect_selected<F: Fn(&str) + 'static>(&self, f: F) {
        let addrs = Rc::clone(&self.addrs);
        self.listbox.connect_row_selected(move |_lb, row| {
            if let Some(row) = row {
                let idx = row.index();
                if idx >= 0 {
                    if let Some(addr) = addrs.borrow().get(idx as usize) {
                        f(addr);
                    }
                }
            }
        });
    }

    /// Sélectionne la première ligne (surbrillance initiale + graphique synchronisé).
    pub fn select_first(&self) {
        if let Some(row) = self.listbox.row_at_index(0) {
            self.listbox.select_row(Some(&row));
        }
    }
}

// Petite fabrique de label de cellule (ellipsé si trop long).
fn label_cell(text: &str, _dim: bool) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.set_xalign(0.0);
    l.set_ellipsize(gtk::pango::EllipsizeMode::End);
    l
}
