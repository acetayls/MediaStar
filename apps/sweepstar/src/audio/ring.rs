//! Tampon circulaire un producteur / un consommateur, sans verrou ni allocation côté
//! producteur (le callback d'entrée audio). Le consommateur (fil de mesure) vide le tampon
//! régulièrement. Une écriture qui ne tient pas est entièrement abandonnée et comptée :
//! les paires (référence, micro) restent toujours alignées.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

pub struct Ring {
    data: Box<[AtomicU32]>,
    mask: usize,
    /// Positions croissantes (jamais ramenées dans le tampon).
    write: AtomicUsize,
    read: AtomicUsize,
    overruns: AtomicU64,
}

impl Ring {
    /// Capacité arrondie à la puissance de deux supérieure.
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.max(2).next_power_of_two();
        Self {
            data: (0..cap).map(|_| AtomicU32::new(0)).collect(),
            mask: cap - 1,
            write: AtomicUsize::new(0),
            read: AtomicUsize::new(0),
            overruns: AtomicU64::new(0),
        }
    }

    pub fn capacity(&self) -> usize {
        self.mask + 1
    }

    /// Producteur : écrit `xs` en entier, ou rien si la place manque (rend `false`).
    pub fn push(&self, xs: &[f32]) -> bool {
        let w = self.write.load(Ordering::Relaxed);
        let r = self.read.load(Ordering::Acquire);
        if self.capacity() - (w - r) < xs.len() {
            self.overruns.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        for (i, &x) in xs.iter().enumerate() {
            self.data[(w + i) & self.mask].store(x.to_bits(), Ordering::Relaxed);
        }
        self.write.store(w + xs.len(), Ordering::Release);
        true
    }

    /// Consommateur : ajoute à `out` tout ce qui est disponible ; rend le nombre lu.
    pub fn pop_into(&self, out: &mut Vec<f32>) -> usize {
        let r = self.read.load(Ordering::Relaxed);
        let w = self.write.load(Ordering::Acquire);
        let n = w - r;
        out.reserve(n);
        for i in 0..n {
            out.push(f32::from_bits(self.data[(r + i) & self.mask].load(Ordering::Relaxed)));
        }
        self.read.store(w, Ordering::Release);
        n
    }

    /// Consommateur : jette ce qui attend.
    pub fn clear(&self) {
        let w = self.write.load(Ordering::Acquire);
        self.read.store(w, Ordering::Release);
    }

    /// Écritures abandonnées depuis le dernier appel.
    pub fn take_overruns(&self) -> u64 {
        self.overruns.swap(0, Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn plein_et_vide() {
        let r = Ring::new(8);
        assert!(r.push(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]));
        assert!(!r.push(&[7.0, 8.0, 9.0]));
        assert_eq!(r.take_overruns(), 1);
        let mut out = Vec::new();
        assert_eq!(r.pop_into(&mut out), 6);
        assert!(r.push(&[7.0, 8.0, 9.0, 10.0, 11.0, 12.0]));
        r.pop_into(&mut out);
        assert_eq!(out, (1..=12).map(|v| v as f32).collect::<Vec<_>>());
    }

    #[test]
    fn deux_fils() {
        let r = Arc::new(Ring::new(1024));
        let p = r.clone();
        let t = std::thread::spawn(move || {
            let mut n = 0u32;
            while n < 100_000 {
                let block: Vec<f32> = (n..n + 64).map(|v| v as f32).collect();
                if p.push(&block) {
                    n += 64;
                } else {
                    std::thread::yield_now();
                }
            }
        });
        let mut out = Vec::new();
        while out.len() < 100_032 {
            r.pop_into(&mut out);
        }
        t.join().unwrap();
        assert!(out.iter().enumerate().all(|(i, &v)| v == i as f32));
    }
}
