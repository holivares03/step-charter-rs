//! Audio: decodificación completa a memoria (symphonia) y reproducción (cpal)
//! con reloj preciso, cambio de velocidad, volumen y ticks de beat/nota.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// Audio decodificado, estéreo intercalado.
pub struct AudioData {
    pub samples: Vec<f32>,
    pub rate: u32,
}

impl AudioData {
    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }
    pub fn duration(&self) -> f64 {
        self.frames() as f64 / self.rate as f64
    }
}

pub fn decode_file(path: &Path) -> Result<AudioData, String> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
    use symphonia::core::errors::Error;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let file = std::fs::File::open(path).map_err(|e| format!("No se pudo abrir el audio: {}", e))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("Formato de audio no soportado: {}", e))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("El archivo no tiene pistas de audio")?;
    let track_id = track.id;
    let mut rate = track.codec_params.sample_rate.unwrap_or(44100);
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Códec no soportado: {}", e))?;

    let mut out: Vec<f32> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(_)) => break,
            Err(Error::ResetRequired) => break,
            Err(e) => {
                if out.is_empty() {
                    return Err(format!("Error leyendo audio: {}", e));
                }
                break;
            }
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                rate = spec.rate;
                let ch = spec.channels.count().max(1);
                let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                buf.copy_interleaved_ref(decoded);
                for frame in buf.samples().chunks(ch) {
                    let l = frame[0];
                    let r = if ch > 1 { frame[1] } else { l };
                    out.push(l);
                    out.push(r);
                }
            }
            Err(Error::DecodeError(_)) => continue,
            Err(Error::IoError(_)) => break,
            Err(e) => return Err(format!("Error decodificando: {}", e)),
        }
    }
    if out.is_empty() {
        return Err("El audio está vacío".into());
    }
    Ok(AudioData { samples: out, rate })
}

/// Picos por bloque para dibujar el waveform.
pub struct Waveform {
    pub block: usize,
    pub rate: u32,
    /// (min_l, max_l, min_r, max_r) por bloque.
    pub peaks: Vec<[f32; 4]>,
}

impl Waveform {
    pub fn build(a: &AudioData) -> Self {
        let block = 128;
        let frames = a.frames();
        let mut peaks = Vec::with_capacity(frames / block + 1);
        let mut f = 0;
        while f < frames {
            let end = (f + block).min(frames);
            let mut p = [0.0f32; 4];
            for i in f..end {
                let l = a.samples[i * 2];
                let r = a.samples[i * 2 + 1];
                p[0] = p[0].min(l);
                p[1] = p[1].max(l);
                p[2] = p[2].min(r);
                p[3] = p[3].max(r);
            }
            peaks.push(p);
            f = end;
        }
        Waveform { block, rate: a.rate, peaks }
    }

    /// Amplitud máxima (izq, der) en el rango de tiempo [t0, t1).
    pub fn range(&self, t0: f64, t1: f64) -> (f32, f32) {
        let bps = self.rate as f64 / self.block as f64;
        let b0 = (t0 * bps).floor();
        let b1 = (t1 * bps).ceil().max(b0 + 1.0);
        if b1 <= 0.0 || b0 >= self.peaks.len() as f64 {
            return (0.0, 0.0);
        }
        let b0 = b0.max(0.0) as usize;
        let b1 = (b1 as usize).min(self.peaks.len());
        let (mut l, mut r) = (0.0f32, 0.0f32);
        for p in &self.peaks[b0..b1] {
            l = l.max(p[1]).max(-p[0]);
            r = r.max(p[3]).max(-p[2]);
        }
        (l, r)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum TickKind {
    Beat,
    Note,
}

struct Voice {
    kind: TickKind,
    /// Fotogramas a esperar antes de empezar (dentro del bloque actual).
    delay: usize,
    age: usize,
}

struct State {
    music: Option<Arc<AudioData>>,
    playing: bool,
    /// Posición en segundos de audio.
    pos: f64,
    speed: f64,
    volume: f32,
    note_ticks: Vec<f64>,
    beat_ticks: Vec<f64>,
    note_tick_on: bool,
    beat_tick_on: bool,
    voices: Vec<Voice>,
    /// Último callback: (instante, posición al inicio del bloque, posición al final).
    clock: Option<(Instant, f64, f64)>,
    device_rate: u32,
}

pub struct AudioPlayer {
    state: Arc<Mutex<State>>,
    _stream: Option<cpal::Stream>,
    pub error: Option<String>,
}

fn render(state: &Mutex<State>, out: &mut [f32], channels: usize) {
    out.fill(0.0);
    let Ok(mut st) = state.lock() else { return };
    let channels = channels.max(1);
    let frames = out.len() / channels;
    let dev_rate = st.device_rate as f64;

    if st.playing {
        let dt = st.speed / dev_rate;
        let t0 = st.pos;
        let t1 = t0 + dt * frames as f64;

        // Ticks que caen en este bloque.
        let mut new_voices = Vec::new();
        for (on, list, kind) in [(st.note_tick_on, &st.note_ticks, TickKind::Note), (st.beat_tick_on, &st.beat_ticks, TickKind::Beat)] {
            if !on {
                continue;
            }
            let start = list.partition_point(|&t| t < t0);
            for &t in &list[start..] {
                if t >= t1 {
                    break;
                }
                let delay = ((t - t0) / dt).max(0.0) as usize;
                new_voices.push(Voice { kind, delay, age: 0 });
            }
        }
        st.voices.extend(new_voices);

        let vol = st.volume;
        if let Some(m) = st.music.clone() {
            let src_rate = m.rate as f64;
            let n = m.frames();
            let mut pos = t0;
            for f in 0..frames {
                let x = pos * src_rate;
                let (l, r) = if x >= 0.0 && x < (n - 1) as f64 {
                    let i = x as usize;
                    let a = (x - i as f64) as f32;
                    let s = &m.samples;
                    (
                        s[i * 2] * (1.0 - a) + s[i * 2 + 2] * a,
                        s[i * 2 + 1] * (1.0 - a) + s[i * 2 + 3] * a,
                    )
                } else {
                    (0.0, 0.0)
                };
                let o = &mut out[f * channels..f * channels + channels];
                if channels == 1 {
                    o[0] = (l + r) * 0.5 * vol;
                } else {
                    o[0] = l * vol;
                    o[1] = r * vol;
                }
                pos += dt;
            }
        }
        st.pos = t1;
        st.clock = Some((Instant::now(), t0, t1));
    }

    // Mezcla de ticks (sintetizados: un "click" senoidal corto).
    let len = (dev_rate * 0.035) as usize;
    for v in st.voices.iter_mut() {
        let freq = if v.kind == TickKind::Note { 1760.0 } else { 1100.0 };
        let amp = if v.kind == TickKind::Note { 0.45 } else { 0.35 };
        let start = v.delay.min(frames);
        for f in start..frames {
            if v.age >= len {
                break;
            }
            let t = v.age as f64 / dev_rate;
            let env = (-(v.age as f64) / (len as f64 * 0.25)).exp();
            let s = ((t * freq * std::f64::consts::TAU).sin() * env * amp) as f32;
            for c in 0..channels.min(2) {
                out[f * channels + c] += s;
            }
            v.age += 1;
        }
        v.delay = v.delay.saturating_sub(frames);
    }
    st.voices.retain(|v| v.age < len);
}

impl AudioPlayer {
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(State {
            music: None,
            playing: false,
            pos: 0.0,
            speed: 1.0,
            volume: 0.8,
            note_ticks: Vec::new(),
            beat_ticks: Vec::new(),
            note_tick_on: false,
            beat_tick_on: false,
            voices: Vec::new(),
            clock: None,
            device_rate: 44100,
        }));
        match Self::open_stream(state.clone()) {
            Ok(stream) => AudioPlayer { state, _stream: Some(stream), error: None },
            Err(e) => AudioPlayer { state, _stream: None, error: Some(e) },
        }
    }

    fn open_stream(state: Arc<Mutex<State>>) -> Result<cpal::Stream, String> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or("No hay dispositivo de salida de audio")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let sample_format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let channels = config.channels as usize;
        state.lock().unwrap().device_rate = config.sample_rate.0;

        let err_fn = |e| eprintln!("error de audio: {}", e);
        let stream = match sample_format {
            cpal::SampleFormat::F32 => {
                let st = state.clone();
                device.build_output_stream(&config, move |data: &mut [f32], _| render(&st, data, channels), err_fn, None)
            }
            cpal::SampleFormat::I16 => {
                let st = state.clone();
                let mut tmp = Vec::new();
                device.build_output_stream(
                    &config,
                    move |data: &mut [i16], _| {
                        tmp.resize(data.len(), 0.0);
                        render(&st, &mut tmp, channels);
                        for (d, s) in data.iter_mut().zip(tmp.iter()) {
                            *d = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
                        }
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::U16 => {
                let st = state.clone();
                let mut tmp = Vec::new();
                device.build_output_stream(
                    &config,
                    move |data: &mut [u16], _| {
                        tmp.resize(data.len(), 0.0);
                        render(&st, &mut tmp, channels);
                        for (d, s) in data.iter_mut().zip(tmp.iter()) {
                            *d = ((s.clamp(-1.0, 1.0) * 0.5 + 0.5) * 65535.0) as u16;
                        }
                    },
                    err_fn,
                    None,
                )
            }
            other => return Err(format!("Formato de salida no soportado: {:?}", other)),
        }
        .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(stream)
    }

    pub fn set_music(&self, music: Option<Arc<AudioData>>) {
        let mut st = self.state.lock().unwrap();
        st.music = music;
    }

    pub fn is_playing(&self) -> bool {
        self.state.lock().unwrap().playing
    }

    pub fn play(&self, from: f64) {
        let mut st = self.state.lock().unwrap();
        st.pos = from;
        st.playing = true;
        st.clock = None;
        st.voices.clear();
    }

    pub fn stop(&self) {
        let mut st = self.state.lock().unwrap();
        st.playing = false;
        st.clock = None;
    }

    /// Tiempo actual de reproducción (interpolado entre callbacks para que sea suave).
    pub fn time(&self) -> f64 {
        let st = self.state.lock().unwrap();
        match st.clock {
            Some((at, t0, t1)) if st.playing => {
                let t = t0 + at.elapsed().as_secs_f64() * st.speed;
                // No adelantarse demasiado si el callback se retrasa.
                t.min(t1 + (t1 - t0))
            }
            _ => st.pos,
        }
    }

    pub fn volume(&self) -> f32 {
        self.state.lock().unwrap().volume
    }
    pub fn set_volume(&self, v: f32) {
        self.state.lock().unwrap().volume = v.clamp(0.0, 1.0);
    }
    pub fn speed(&self) -> f64 {
        self.state.lock().unwrap().speed
    }
    pub fn set_speed(&self, s: f64) {
        let mut st = self.state.lock().unwrap();
        // Mantener continuidad del reloj al cambiar la velocidad.
        if let (true, Some((at, t0, _))) = (st.playing, st.clock) {
            st.pos = t0 + at.elapsed().as_secs_f64() * st.speed;
            st.clock = None;
        }
        st.speed = s.clamp(0.1, 3.0);
    }

    pub fn set_ticks(&self, note_ticks: Vec<f64>, beat_ticks: Vec<f64>) {
        let mut st = self.state.lock().unwrap();
        st.note_ticks = note_ticks;
        st.beat_ticks = beat_ticks;
    }
    pub fn ticks_enabled(&self) -> (bool, bool) {
        let st = self.state.lock().unwrap();
        (st.beat_tick_on, st.note_tick_on)
    }
    pub fn set_ticks_enabled(&self, beat: bool, note: bool) {
        let mut st = self.state.lock().unwrap();
        st.beat_tick_on = beat;
        st.note_tick_on = note;
    }
}
