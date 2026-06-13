// Module "model" — structures de données (= models.py + calcul d'état de monitor.py)
//
// Concepts Rust introduits :
//   - les enum (Status) : des types à valeurs fermées, bien plus sûrs que des str
//   - impl : où l'on attache des méthodes à une struct (≈ les méthodes d'une classe)
//   - les itérateurs (.iter().filter().map()...) : style fonctionnel, sans boucle manuelle
//   - &self vs &mut self : lecture seule vs mutation

use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

// Constantes de configuration (viendront d'un fichier de config à l'étape 5).
pub const WINDOW: usize = 120; // taille de la fenêtre glissante d'échantillons
pub const DEGRADED_LOSS: f64 = 5.0; // % de perte au-delà duquel on est "dégradé"
pub const DEGRADED_LATENCY: f64 = 120.0; // latence moyenne (ms) idem

/// État d'un hôte. En Python c'étaient des chaînes ("online", "offline"…),
/// fragiles (faute de frappe = bug silencieux). En Rust c'est un enum : le
/// compilateur connaît les 4 seules valeurs possibles et vérifie qu'on les
/// traite toutes. `#[derive(...)]` génère égalité, copie et affichage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Online,
    Degraded,
    Offline,
    Unknown,
}

/// Une mesure : une latence (None = paquet perdu). L'horodatage des points
/// vit dans le stockage SQLite, pas ici (la fenêtre glissante en mémoire ne
/// sert qu'aux stats live, qui n'en ont pas besoin).
#[derive(Debug, Clone, Copy)]
pub struct Sample {
    pub latency: Option<f64>,
}

impl Sample {
    pub fn lost(&self) -> bool {
        self.latency.is_none()
    }
}

/// Seuils du score de qualité (modifiables plus tard via les Paramètres).
#[derive(Debug, Clone, Copy)]
pub struct QualityCfg {
    pub good_latency: f64,
    pub good_jitter: f64,
    pub loss_per_point: f64,
}

// Default = l'équivalent des valeurs par défaut d'une dataclass Python.
impl Default for QualityCfg {
    fn default() -> Self {
        Self {
            good_latency: 50.0,
            good_jitter: 15.0,
            loss_per_point: 5.0,
        }
    }
}

/// Un hôte surveillé et son état courant en mémoire.
pub struct Host {
    pub address: String,
    pub name: String,
    pub recent: VecDeque<Sample>, // VecDeque = file à double extrémité (comme deque Python)
    pub status: Status,
    pub quality_cfg: QualityCfg,
    // Seuils "dégradé" (modifiables via les Paramètres ; défauts = les constantes).
    pub degraded_loss: f64,
    pub degraded_latency: f64,
}

impl Host {
    pub fn new(address: &str, name: &str) -> Self {
        let name = if name.is_empty() {
            address.to_string()
        } else {
            name.to_string()
        };
        Self {
            address: address.to_string(),
            name,
            recent: VecDeque::with_capacity(WINDOW),
            status: Status::Unknown,
            quality_cfg: QualityCfg::default(),
            degraded_loss: DEGRADED_LOSS,
            degraded_latency: DEGRADED_LATENCY,
        }
    }

    /// Ajoute un échantillon (et recalcule l'état). `&mut self` = on modifie l'hôte.
    pub fn add(&mut self, sample: Sample) {
        if self.recent.len() == WINDOW {
            self.recent.pop_front(); // on jette le plus ancien (fenêtre glissante)
        }
        self.recent.push_back(sample);
        self.status = self.compute_status();
    }

    /// Dernière latence connue (en remontant). `&self` = lecture seule.
    pub fn last_latency(&self) -> Option<f64> {
        // .iter().rev() parcourt à l'envers ; find_map renvoie le 1er Some trouvé.
        self.recent.iter().rev().find_map(|s| s.latency)
    }

    /// Pourcentage de pertes sur la fenêtre.
    pub fn loss_pct(&self) -> f64 {
        if self.recent.is_empty() {
            return 0.0;
        }
        let lost = self.recent.iter().filter(|s| s.lost()).count();
        100.0 * lost as f64 / self.recent.len() as f64
    }

    /// Gigue = moyenne des écarts absolus entre latences consécutives.
    pub fn jitter(&self) -> Option<f64> {
        // On collecte d'abord les latences valides dans un Vec<f64>.
        let lats: Vec<f64> = self.recent.iter().filter_map(|s| s.latency).collect();
        if lats.len() < 2 {
            return None;
        }
        let mut sum = 0.0;
        for i in 1..lats.len() {
            sum += (lats[i] - lats[i - 1]).abs();
        }
        Some(sum / (lats.len() - 1) as f64)
    }

    /// Latence moyenne sur la fenêtre.
    pub fn avg_latency(&self) -> Option<f64> {
        let lats: Vec<f64> = self.recent.iter().filter_map(|s| s.latency).collect();
        if lats.is_empty() {
            return None;
        }
        Some(lats.iter().sum::<f64>() / lats.len() as f64)
    }

    /// Les valeurs pour la sparkline (latences, avec trous None pour les pertes).
    pub fn sparkline(&self) -> Vec<Option<f64>> {
        self.recent.iter().map(|s| s.latency).collect()
    }

    /// Recalcule l'état (logique identique à monitor.py::_compute_status).
    fn compute_status(&self) -> Status {
        if self.recent.is_empty() {
            return Status::Unknown;
        }
        // Les 5 derniers tous perdus → hors ligne.
        let tail_all_lost = self.recent.iter().rev().take(5).all(|s| s.lost());
        if tail_all_lost {
            return Status::Offline;
        }
        let loss = self.loss_pct();
        let lat = self.avg_latency();
        if loss >= self.degraded_loss || lat.is_some_and(|l| l >= self.degraded_latency) {
            return Status::Degraded;
        }
        Status::Online
    }

    /// Score de qualité 0..5 (logique identique à models.py::quality).
    pub fn quality(&self) -> i32 {
        if self.recent.is_empty() {
            return 0;
        }
        if self.status == Status::Offline || self.loss_pct() >= 100.0 {
            return 0;
        }
        let qc = &self.quality_cfg;
        let loss_pp = if qc.loss_per_point == 0.0 {
            5.0
        } else {
            qc.loss_per_point
        };

        let mut score = 5.0_f64;
        let loss = self.loss_pct();
        if loss > 0.0 {
            score -= (loss / loss_pp).min(4.0); // -1 pt par tranche de perte
        }
        if let Some(lat) = self.avg_latency() {
            if qc.good_latency > 0.0 && lat > qc.good_latency {
                score -= ((lat - qc.good_latency) / qc.good_latency).min(2.0);
            }
        }
        if let Some(jit) = self.jitter() {
            if qc.good_jitter > 0.0 && jit > qc.good_jitter {
                score -= ((jit - qc.good_jitter) / qc.good_jitter).min(2.0);
            }
        }
        // ceil puis on borne entre 0 et 5. clamp = max(min(...)) en une fois.
        (score.ceil() as i32).clamp(0, 5)
    }

    /// Texte de l'infobulle détaillant le score (en français pour l'instant ;
    /// l'i18n viendra à l'étape 5).
    pub fn quality_tooltip(&self) -> String {
        let lat_s = self
            .avg_latency()
            .map_or_else(|| "—".to_string(), |l| format!("{l:.0} ms"));
        let jit_s = self
            .jitter()
            .map_or_else(|| "—".to_string(), |j| format!("{j:.0} ms"));
        let qc = &self.quality_cfg;
        format!(
            "Qualité : {}/5\n\
             Latence moyenne : {}  (idéal ≤ {:.0} ms)\n\
             Perte : {:.0} %  (−1 pt / {:.0} %)\n\
             Gigue : {}  (idéal ≤ {:.0} ms)\n\
             Sur les {} derniers pings",
            self.quality(),
            lat_s,
            qc.good_latency,
            self.loss_pct(),
            qc.loss_per_point,
            jit_s,
            qc.good_jitter,
            self.recent.len()
        )
    }
}

/// Horodatage courant en secondes (epoch), comme time.time() en Python.
pub fn now_ts() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}
