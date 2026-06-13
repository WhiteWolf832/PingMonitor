// Module "ping" — le moteur de mesure (équivalent de monitor.py côté Python).
//
// Concepts Rust introduits :
//   - les modules (ce fichier = module `ping`, rattaché par `mod ping;` dans main.rs)
//   - `pub` : ce qui est exporté hors du module
//   - les threads natifs (std::thread)
//   - les channels (tuyaux) pour faire remonter les résultats au thread principal
//   - le mot-clé `move` qui transfère l'ownership dans une closure de thread

use std::process::Command;
use std::thread;
use std::time::Duration;

use async_channel::Sender;

/// Un message envoyé par un thread de ping vers l'interface.
///
/// `#[derive(Debug, Clone)]` génère automatiquement deux capacités :
///   - Debug : pouvoir l'afficher avec {:?} (pratique pour déboguer)
///   - Clone : pouvoir le dupliquer
/// C'est le pendant des dunder methods Python, mais généré par le compilateur.
#[derive(Debug, Clone)]
pub struct Update {
    pub host: String,
    pub latency_ms: Option<f64>, // Some(latence) si en ligne, None si injoignable
}

/// Pingue `host` une seule fois. Renvoie la latence en ms, ou None.
///
/// Note le `?` sur `.ok()?` : si `ping` ne se lance même pas, on renvoie None
/// immédiatement. Ici `?` fonctionne sur Option (pas seulement Result).
pub fn ping_once(host: &str) -> Option<f64> {
    let output = Command::new("ping")
        .args(["-c", "1", "-W", "1", host])
        .output()
        .ok()?; // si échec de lancement → None

    let text = String::from_utf8_lossy(&output.stdout);
    for token in text.split_whitespace() {
        if let Some(value) = token.strip_prefix("time=") {
            return value.parse::<f64>().ok();
        }
    }
    None
}

/// Démarre la surveillance : un thread par hôte, qui pingue en boucle et
/// envoie chaque résultat dans le channel.
///
/// `hosts: Vec<String>` = on PREND possession de la liste (on va la découper et
/// distribuer chaque hôte à un thread, donc on doit la posséder).
/// `sender: Sender<Update>` = le bout "écriture" du tuyau.
pub fn start_monitor(hosts: Vec<String>, interval: Duration, sender: Sender<Update>) {
    // `for host in hosts` consomme la Vec : chaque `host` est une String possédée
    // qu'on va déplacer (move) dans son thread.
    for host in hosts {
        // Chaque thread a besoin de SA copie du bout-écriture du tuyau.
        // .clone() sur un Sender est bon marché (partage le même tuyau interne).
        let tx = sender.clone();

        // thread::spawn lance un vrai thread système. La closure `move || {...}`
        // PREND possession de `host` et `tx` : ils vivent désormais dans le thread.
        // Sans `move`, Rust refuserait — il ne sait pas combien de temps le thread
        // vit, donc il ne peut pas laisser le thread emprunter des variables locales.
        thread::spawn(move || {
            loop {
                let latency = ping_once(&host);

                let msg = Update {
                    host: host.clone(), // on clone car la boucle réutilise `host`
                    latency_ms: latency,
                };

                // send_blocking = version synchrone de l'envoi (on est dans un
                // thread classique, pas dans du code async).
                // Si l'envoi échoue, c'est que le récepteur (l'UI) a disparu →
                // la fenêtre est fermée, donc on arrête le thread proprement.
                if tx.send_blocking(msg).is_err() {
                    break;
                }

                // Pause avant le prochain ping (comme l'intervalle en Python).
                thread::sleep(interval);
            }
        });
    }
}
