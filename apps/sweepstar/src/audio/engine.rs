//! Périphériques audio : un fil dédié (`audio-host`) possède les flux cpal (non `Send` sur
//! certaines plateformes) et les rouvre sur demande, sans figer l'interface.
//!
//! - Sortie : `OutputState` (générateurs, sweep), sur les canaux choisis.
//! - Entrée : crêtes de chaque canal pour les vu-mètres, et paires (référence, micro) copiées
//!   dans le tampon circulaire, sans verrou ni allocation.
//!
//! Entrée et sortie doivent tourner à la même fréquence d'échantillonnage (pas de
//! rééchantillonnage dans une chaîne de mesure). Sur la même carte, elles partagent l'horloge.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, FromSample, SampleFormat, SizedSample, StreamConfig};
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use star_core::i18n::tr;

use super::ring::Ring;
use super::signal::{OutCmd, OutShared, OutputState, Source, MAX_BLOCK};

/// Canaux d'entrée suivis par les vu-mètres.
pub const MAX_INPUTS: usize = 32;
/// Pas de canal de référence.
pub const NO_CHANNEL: usize = usize::MAX;
/// Capacité du tampon d'entrée : 8 s de paires à 48 kHz.
const RING_SAMPLES: usize = 1 << 20;

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceInfo {
    pub host: String,
    pub output: String,
    pub input: Option<String>,
    pub out_channels: usize,
    pub in_channels: usize,
    pub sample_rate: u32,
}

#[derive(Debug, Clone)]
pub struct DeviceDesc {
    pub name: String,
    pub channels: usize,
    pub is_default: bool,
}

/// État de l'entrée partagé avec l'interface et le fil de mesure.
pub struct InShared {
    /// Crêtes par canal depuis la dernière lecture (bits d'un f32).
    pub peaks: [AtomicU32; MAX_INPUTS],
    /// Canal de la boucle de référence (`NO_CHANNEL` : aucun).
    pub ref_channel: AtomicUsize,
    pub mic_channel: AtomicUsize,
    /// Images reçues depuis l'ouverture.
    pub frames: AtomicU64,
}

impl InShared {
    fn new() -> Self {
        Self { peaks: std::array::from_fn(|_| AtomicU32::new(0)), ref_channel: AtomicUsize::new(NO_CHANNEL), mic_channel: AtomicUsize::new(0), frames: AtomicU64::new(0) }
    }

    pub fn take_peak(&self, ch: usize) -> f32 {
        self.peaks.get(ch).map_or(0.0, |p| f32::from_bits(p.swap(0, Ordering::Relaxed)))
    }
}

pub fn host_names() -> Vec<String> {
    cpal::available_hosts().iter().map(|h| h.name().to_string()).collect()
}

fn find_host(name: Option<&str>) -> cpal::Host {
    name.and_then(|n| cpal::available_hosts().into_iter().find(|h| h.name() == n))
        .and_then(|id| cpal::host_from_id(id).ok())
        .unwrap_or_else(cpal::default_host)
}

pub fn list_outputs(host: Option<&str>) -> Vec<DeviceDesc> {
    let host = find_host(host);
    let default = host.default_output_device().map(|d| d.to_string());
    let Ok(devices) = host.output_devices() else { return Vec::new() };
    devices
        .map(|d| {
            let name = d.to_string();
            let channels = d.supported_output_configs().map(|it| it.map(|c| c.channels() as usize).max().unwrap_or(0)).unwrap_or(0);
            DeviceDesc { is_default: default.as_deref() == Some(name.as_str()), channels, name }
        })
        .collect()
}

pub fn list_inputs(host: Option<&str>) -> Vec<DeviceDesc> {
    let host = find_host(host);
    let default = host.default_input_device().map(|d| d.to_string());
    let Ok(devices) = host.input_devices() else { return Vec::new() };
    devices
        .map(|d| {
            let name = d.to_string();
            let channels = d.supported_input_configs().map(|it| it.map(|c| c.channels() as usize).max().unwrap_or(0)).unwrap_or(0);
            DeviceDesc { is_default: default.as_deref() == Some(name.as_str()), channels, name }
        })
        .collect()
}

/// Configuration avec le plus de canaux à la fréquence voulue (48 kHz ou 44,1 kHz sinon).
fn pick<I: Iterator<Item = cpal::SupportedStreamConfigRange>>(ranges: I, wanted: Option<u32>) -> Option<(StreamConfig, SampleFormat)> {
    ranges
        .filter_map(|r| {
            let rates: &[u32] = match wanted {
                Some(w) => &[w][..],
                None => &[48_000, 44_100][..],
            };
            let rate = rates.iter().copied().find(|&sr| r.min_sample_rate() <= sr && sr <= r.max_sample_rate());
            let rate = match (rate, wanted) {
                (Some(sr), _) => sr,
                (None, Some(_)) => return None,
                (None, None) => r.max_sample_rate(),
            };
            let rate_score = match rate {
                48_000 => 2,
                44_100 => 1,
                _ => 0,
            };
            let fmt_score = (r.sample_format() == SampleFormat::F32) as u8;
            Some(((rate_score, r.channels(), fmt_score), r.with_sample_rate(rate)))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, c)| (c.config(), c.sample_format()))
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct OpenRequest {
    pub host: Option<String>,
    pub output: Option<String>,
    pub input: Option<String>,
    pub buffer_size: Option<u32>,
}

enum HostMsg {
    Open(OpenRequest, Sender<Result<DeviceInfo, String>>),
}

pub struct AudioEngine {
    host_tx: Sender<HostMsg>,
    pub out_cmd: Sender<OutCmd>,
    pub out: Arc<OutShared>,
    pub input: Arc<InShared>,
    pub ring: Arc<Ring>,
    /// Erreurs remontées par les flux (périphérique débranché…).
    pub stream_errors: Receiver<String>,
}

impl AudioEngine {
    pub fn new(gain: f32, channel_mask: u64) -> Self {
        let (cmd_tx, cmd_rx) = bounded::<OutCmd>(64);
        let (garbage_tx, garbage_rx) = bounded::<Box<Source>>(64);
        let (err_tx, err_rx) = bounded::<String>(64);
        let (host_tx, host_rx) = unbounded::<HostMsg>();
        let out = Arc::new(OutShared::new(gain, channel_mask));
        let input = Arc::new(InShared::new());
        let ring = Arc::new(Ring::new(RING_SAMPLES));

        thread::Builder::new().name("audio-garbage".into()).spawn(move || for _ in garbage_rx {}).expect("thread audio-garbage");

        let (o, i, r) = (out.clone(), input.clone(), ring.clone());
        thread::Builder::new()
            .name("audio-host".into())
            .spawn(move || {
                let mut streams: Vec<cpal::Stream> = Vec::new();
                for msg in host_rx {
                    streams.clear();
                    let HostMsg::Open(req, reply) = msg;
                    let res = open(&req, &cmd_rx, &garbage_tx, &o, &i, &r, &err_tx);
                    let _ = reply.send(res.map(|(s, info)| {
                        streams = s;
                        info
                    }));
                }
            })
            .expect("thread audio-host");

        Self { host_tx, out_cmd: cmd_tx, out, input, ring, stream_errors: err_rx }
    }

    /// Ouvre (ou rouvre) les périphériques sans attendre ; la réponse arrive sur le récepteur.
    pub fn request_open(&self, req: OpenRequest) -> Receiver<Result<DeviceInfo, String>> {
        let (tx, rx) = bounded(1);
        if self.host_tx.send(HostMsg::Open(req, tx.clone())).is_err() {
            let _ = tx.send(Err(tr("moteur audio arrêté", "audio engine stopped").to_string()));
        }
        rx
    }

    pub fn send(&self, cmd: OutCmd) {
        let _ = self.out_cmd.try_send(cmd);
    }
}

fn open(
    req: &OpenRequest,
    cmd_rx: &Receiver<OutCmd>,
    garbage: &Sender<Box<Source>>,
    out: &Arc<OutShared>,
    input: &Arc<InShared>,
    ring: &Arc<Ring>,
    errors: &Sender<String>,
) -> Result<(Vec<cpal::Stream>, DeviceInfo), String> {
    let host = find_host(req.host.as_deref());
    let out_dev = match &req.output {
        Some(n) => host.output_devices().ok().and_then(|mut it| it.find(|d| &d.to_string() == n)),
        None => host.default_output_device(),
    }
    .ok_or_else(|| tr("sortie audio introuvable", "audio output not found").to_string())?;
    let out_ranges = out_dev.supported_output_configs().map_err(|e| e.to_string())?;
    let (mut out_cfg, out_fmt) = pick(out_ranges, None).ok_or_else(|| tr("aucune configuration de sortie", "no output configuration").to_string())?;
    let rate = out_cfg.sample_rate;

    let in_dev = match &req.input {
        Some(n) => host.input_devices().ok().and_then(|mut it| it.find(|d| &d.to_string() == n)),
        None => host.default_input_device(),
    };
    let in_cfg = match &in_dev {
        Some(d) => {
            let ranges = d.supported_input_configs().map_err(|e| e.to_string())?;
            Some(pick(ranges, Some(rate)).ok_or_else(|| format!("{} {rate} Hz", tr("l'entrée ne tourne pas à", "the input does not run at")))?)
        }
        None => None,
    };

    let mut streams = Vec::new();
    let on_err = |errors: &Sender<String>| {
        let errors = errors.clone();
        move |e: cpal::Error| {
            let _ = errors.try_send(e.to_string());
        }
    };

    // Sortie (avec repli sur la taille de tampon du pilote).
    let build_out = |cfg: StreamConfig| {
        let state = OutputState::new(cfg.sample_rate, cmd_rx.clone(), garbage.clone(), out.clone());
        match out_fmt {
            SampleFormat::F32 => build_output::<f32>(&out_dev, cfg, state, on_err(errors)),
            SampleFormat::F64 => build_output::<f64>(&out_dev, cfg, state, on_err(errors)),
            SampleFormat::I16 => build_output::<i16>(&out_dev, cfg, state, on_err(errors)),
            SampleFormat::I32 => build_output::<i32>(&out_dev, cfg, state, on_err(errors)),
            SampleFormat::U16 => build_output::<u16>(&out_dev, cfg, state, on_err(errors)),
            other => Err(format!("{} : {other:?}", tr("format d'échantillon non géré", "unsupported sample format"))),
        }
    };
    let out_stream = match req.buffer_size {
        Some(n) => {
            out_cfg.buffer_size = BufferSize::Fixed(n);
            build_out(out_cfg).or_else(|_| {
                out_cfg.buffer_size = BufferSize::Default;
                build_out(out_cfg)
            })?
        }
        None => build_out(out_cfg)?,
    };
    streams.push(out_stream);

    let mut in_channels = 0;
    if let (Some(dev), Some((mut cfg, fmt))) = (&in_dev, in_cfg) {
        in_channels = cfg.channels as usize;
        input.frames.store(0, Ordering::Relaxed);
        let build_in = |cfg: StreamConfig| match fmt {
            SampleFormat::F32 => build_input::<f32>(dev, cfg, input.clone(), ring.clone(), on_err(errors)),
            SampleFormat::F64 => build_input::<f64>(dev, cfg, input.clone(), ring.clone(), on_err(errors)),
            SampleFormat::I16 => build_input::<i16>(dev, cfg, input.clone(), ring.clone(), on_err(errors)),
            SampleFormat::I32 => build_input::<i32>(dev, cfg, input.clone(), ring.clone(), on_err(errors)),
            SampleFormat::U16 => build_input::<u16>(dev, cfg, input.clone(), ring.clone(), on_err(errors)),
            other => Err(format!("{} : {other:?}", tr("format d'échantillon non géré", "unsupported sample format"))),
        };
        let s = match req.buffer_size {
            Some(n) => {
                cfg.buffer_size = BufferSize::Fixed(n);
                build_in(cfg).or_else(|_| {
                    cfg.buffer_size = BufferSize::Default;
                    build_in(cfg)
                })?
            }
            None => build_in(cfg)?,
        };
        streams.push(s);
    }
    for s in &streams {
        s.play().map_err(|e| format!("{} : {e}", tr("démarrage du flux impossible", "cannot start stream")))?;
    }
    let info = DeviceInfo {
        host: host.id().name().to_string(),
        output: out_dev.to_string(),
        input: in_dev.as_ref().map(|d| d.to_string()),
        out_channels: out_cfg.channels as usize,
        in_channels,
        sample_rate: rate,
    };
    Ok((streams, info))
}

fn build_output<T>(device: &cpal::Device, config: StreamConfig, mut state: OutputState, on_err: impl FnMut(cpal::Error) + Send + 'static) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    let mut scratch = vec![0f32; MAX_BLOCK * channels.max(1)];
    device
        .build_output_stream::<T, _, _>(
            config,
            move |data: &mut [T], _| {
                for chunk in data.chunks_mut(scratch.len()) {
                    let buf = &mut scratch[..chunk.len()];
                    state.process(buf, channels);
                    for (o, &s) in chunk.iter_mut().zip(buf.iter()) {
                        // Écrêtage juste sous la pleine échelle : les conversions entières
                        // débordent à +1,0.
                        *o = T::from_sample(if s.is_finite() { s.clamp(-1.0, 0.999_999) } else { 0.0 });
                    }
                }
            },
            on_err,
            None,
        )
        .map_err(|e| format!("{} : {e}", tr("ouverture de la sortie impossible", "cannot open output")))
}

fn build_input<T>(device: &cpal::Device, config: StreamConfig, shared: Arc<InShared>, ring: Arc<Ring>, on_err: impl FnMut(cpal::Error) + Send + 'static) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = (config.channels as usize).max(1);
    let mut pairs = vec![0f32; MAX_BLOCK * 2];
    let mut peaks = [0f32; MAX_INPUTS];
    device
        .build_input_stream::<T, _, _>(
            config,
            move |data: &[T], _| {
                let rc = shared.ref_channel.load(Ordering::Relaxed);
                let mc = shared.mic_channel.load(Ordering::Relaxed);
                peaks.fill(0.0);
                for block in data.chunks(MAX_BLOCK * channels) {
                    let n = block.len() / channels;
                    for (f, frame) in block.chunks_exact(channels).enumerate() {
                        for (c, s) in frame.iter().enumerate().take(MAX_INPUTS) {
                            let v: f32 = s.to_sample::<f32>();
                            peaks[c] = peaks[c].max(v.abs());
                        }
                        let get = |c: usize| if c < channels { frame[c].to_sample::<f32>() } else { 0.0 };
                        pairs[2 * f] = if rc == NO_CHANNEL { 0.0 } else { get(rc) };
                        pairs[2 * f + 1] = get(mc);
                    }
                    ring.push(&pairs[..2 * n]);
                    shared.frames.fetch_add(n as u64, Ordering::Relaxed);
                }
                for (a, &p) in shared.peaks.iter().zip(peaks.iter()) {
                    if p > f32::from_bits(a.load(Ordering::Relaxed)) {
                        a.store(p.to_bits(), Ordering::Relaxed);
                    }
                }
            },
            on_err,
            None,
        )
        .map_err(|e| format!("{} : {e}", tr("ouverture de l'entrée impossible", "cannot open input")))
}
