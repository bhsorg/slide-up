use anyhow::{Context, Result};
use ndarray::{Array1, Array2, Array3, Axis};
use ort::ep::CPU;
#[cfg(target_os = "windows")]
use ort::ep::DirectML;
#[cfg(target_os = "macos")]
use ort::ep::CoreML;
#[cfg(target_os = "linux")]
use ort::ep::CUDA;

// Builder errors carry a `SessionBuilder`, which is not Send/Sync and therefore
// cannot be converted into `anyhow::Error` directly. Erase the payload first.
fn ort_builder_err(error: ort::Error<ort::session::builder::SessionBuilder>) -> ort::Error<()> {
    ort::Error::from(error)
}

fn tensor_f32_2d(array: Array2<f32>) -> Result<Tensor<f32>> {
    let shape = array.shape().to_vec();
    Ok(Tensor::from_array((shape, array.into_raw_vec_and_offset().0))?)
}

fn tensor_f32_3d(array: Array3<f32>) -> Result<Tensor<f32>> {
    let shape = array.shape().to_vec();
    Ok(Tensor::from_array((shape, array.into_raw_vec_and_offset().0))?)
}

fn tensor_i64_1d(array: Array1<i64>) -> Result<Tensor<i64>> {
    let shape = array.shape().to_vec();
    Ok(Tensor::from_array((shape, array.into_raw_vec_and_offset().0))?)
}

fn tensor_i32_1d(array: Array1<i32>) -> Result<Tensor<i32>> {
    let shape = array.shape().to_vec();
    Ok(Tensor::from_array((shape, array.into_raw_vec_and_offset().0))?)
}
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use rustfft::{num_complex::Complex, FftPlanner};
use std::collections::HashMap;
use std::f32::consts::PI;
use std::fs;
use std::path::Path;

pub struct OnnxCaptionEngine {
    vad_session: Session,
    stt_session: Session,
    vocab: HashMap<i64, String>,
    mel_filters: Array2<f32>,
    pub provider_name: String,
}

impl OnnxCaptionEngine {
    /// Auto GPU (DirectML/CoreML/CUDA) + CPU Fallback ke saath dono ONNX models load karta hai
    pub fn new(model_dir: &Path) -> Result<Self> {
        let vad_path = model_dir.join("silero_vad.onnx");
        let stt_path = model_dir.join("model.int8.onnx");
        let tokens_path = model_dir.join("tokens.txt");

        let available_cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        let optimal_threads = (available_cores / 2).clamp(1, 4);

        // 1. Silero VAD hamesha CPU par chalayein (2MB model hai, <1ms lagta hai)
        let vad_session = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level1).map_err(ort_builder_err)?
            .with_intra_threads(1).map_err(ort_builder_err)?
            .commit_from_file(&vad_path)
            .context("Failed to load silero_vad.onnx")?;

        // 2. SenseVoice STT ke liye pehle GPU try karein, fail hone par CPU fallback
        let (stt_session, provider_name) = Self::build_stt_session(&stt_path, optimal_threads)?;

        // 3. tokens.txt load karein
        let vocab = Self::load_vocab(&tokens_path)?;

        // 4. 80-bin Mel Filterbank precompute karein (16kHz, 400 FFT)
        let mel_filters = Self::create_mel_filterbank(16000, 400, 80);

        Ok(Self {
            vad_session,
            stt_session,
            vocab,
            mel_filters,
            provider_name,
        })
    }

    fn build_stt_session(model_path: &Path, threads: usize) -> Result<(Session, String)> {
        // GPU Provider try karein (OS ke hisaab se)
        #[cfg(target_os = "windows")]
        let gpu_attempt = Session::builder()?
            .with_execution_providers([DirectML::default().build()]).map_err(ort_builder_err)?
            .with_optimization_level(GraphOptimizationLevel::Level1).map_err(ort_builder_err)?
            .commit_from_file(model_path);
        #[cfg(target_os = "windows")]
        let gpu_label = "DirectML (GPU)";

        #[cfg(target_os = "macos")]
        let gpu_attempt = Session::builder()?
            .with_execution_providers([CoreML::default().build()]).map_err(ort_builder_err)?
            .with_optimization_level(GraphOptimizationLevel::Level1).map_err(ort_builder_err)?
            .commit_from_file(model_path);
        #[cfg(target_os = "macos")]
        let gpu_label = "CoreML (GPU)";

        #[cfg(target_os = "linux")]
        let gpu_attempt = Session::builder()?
            .with_execution_providers([CUDA::default().build()]).map_err(ort_builder_err)?
            .with_optimization_level(GraphOptimizationLevel::Level1).map_err(ort_builder_err)?
            .commit_from_file(model_path);
        #[cfg(target_os = "linux")]
        let gpu_label = "CUDA (GPU)";

        if let Ok(session) = gpu_attempt {
            return Ok((session, gpu_label.to_string()));
        }

        // Fallback: Low-CPU thread-limited session
        let cpu_session = Session::builder()?
            .with_execution_providers([CPU::default().build()]).map_err(ort_builder_err)?
            .with_optimization_level(GraphOptimizationLevel::Level1).map_err(ort_builder_err)?
            .with_intra_threads(threads).map_err(ort_builder_err)?
            .commit_from_file(model_path)
            .context("Failed to load STT ONNX model on CPU")?;

        Ok((cpu_session, format!("CPU ({} threads)", threads)))
    }

    /// Silero VAD check: 512 samples (32ms @ 16kHz) par speech probability return karta hai
    pub fn is_speech(
        &mut self,
        frame_512: &[f32],
        state: &mut Array3<f32>, // Shape: [2, 1, 128]
    ) -> Result<bool> {
        let input = Array2::from_shape_vec((1, 512), frame_512.to_vec())?;
        let sr = Array1::from_vec(vec![16000i64]);

        let outputs = self.vad_session.run(ort::inputs![
            "input" => tensor_f32_2d(input)?,
            "state" => tensor_f32_3d(state.clone())?,
            "sr" => tensor_i64_1d(sr)?,
        ])?;

        let prob = outputs["output"].try_extract_tensor::<f32>()?.1[0];
        let new_state = outputs["stateN"].try_extract_tensor::<f32>()?;
        *state = Array3::from_shape_vec((2, 1, 128), new_state.1.to_vec())?;

        Ok(prob > 0.45) // 0.45 threshold background music/noise ko block karta hai
    }

    /// Speech chunk (16kHz f32 PCM) ko Hindi / Hinglish / English text me convert karta hai
    pub fn transcribe_chunk(&mut self, samples: &[f32]) -> Result<String> {
        if samples.len() < 1600 {
            return Ok(String::new());
        }

        // 1. Log-Mel Spectrogram [T, 80] nikalein
        let mel = self.compute_log_mel(samples);

        // 2. LFR (Low Frame Rate: stack 7 frames, stride 6) -> [T', 560]
        let lfr_feats = Self::apply_lfr(&mel, 7, 6);
        let num_frames = lfr_feats.nrows() as i32;
        if num_frames == 0 {
            return Ok(String::new());
        }

        // Batch dimension add karein: [1, T', 560]
        let x = lfr_feats.insert_axis(Axis(0));
        let x_len = Array1::from_vec(vec![num_frames]);
        // SenseVoice inputs: language (0 = auto), text_norm (15 = with inverse text norm)
        let language = Array1::from_vec(vec![0i32]);
        let text_norm = Array1::from_vec(vec![15i32]);

        // 3. ONNX Inference run karein
        let outputs = self.stt_session.run(ort::inputs![
            "x" => tensor_f32_3d(x)?,
            "x_length" => tensor_i32_1d(x_len)?,
            "language" => tensor_i32_1d(language)?,
            "text_norm" => tensor_i32_1d(text_norm)?,
        ])?;

        // 4. CTC Greedy Decoding (logits shape: [1, T', vocab_size])
        let (shape, logits) = outputs["logits"].try_extract_tensor::<f32>()?;
        let time_steps = shape[1] as usize;
        let vocab_size = shape[2] as usize;

        let mut decoded_text = String::new();
        let mut prev_id: i64 = -1;
        let blank_id: i64 = 0;

        for t in 0..time_steps {
            let row_start = t * vocab_size;
            let row = &logits[row_start..row_start + vocab_size];

            // Argmax
            let (best_id, _) = row
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .unwrap_or((0, &0.0));
            let token_id = best_id as i64;

            if token_id != blank_id && token_id != prev_id {
                if let Some(piece) = self.vocab.get(&token_id) {
                    // Special tags jaise <|hi|>, <|en|>, <|NEUTRAL|> skip karein
                    if !piece.starts_with("<|") {
                        decoded_text.push_str(&piece.replace('▁', " "));
                    }
                }
            }
            prev_id = token_id;
        }

        Ok(decoded_text.trim().to_string())
    }

    fn load_vocab(path: &Path) -> Result<HashMap<i64, String>> {
        let content = fs::read_to_string(path).context("Failed to read tokens.txt")?;
        let mut map = HashMap::new();
        for line in content.lines() {
            let mut parts = line.split_whitespace();
            if let (Some(token), Some(id_str)) = (parts.next(), parts.next()) {
                if let Ok(id) = id_str.parse::<i64>() {
                    map.insert(id, token.to_string());
                }
            }
        }
        Ok(map)
    }

    fn create_mel_filterbank(sr: usize, n_fft: usize, n_mels: usize) -> Array2<f32> {
        let n_freqs = n_fft / 2 + 1;
        let f_min = 20.0f32;
        let f_max = (sr / 2) as f32;
        let mel_min = 2595.0 * (1.0 + f_min / 700.0).log10();
        let mel_max = 2595.0 * (1.0 + f_max / 700.0).log10();

        let mel_points: Vec<f32> = (0..=n_mels + 1)
            .map(|i| mel_min + (mel_max - mel_min) * (i as f32) / ((n_mels + 1) as f32))
            .collect();
        let hz_points: Vec<f32> = mel_points
            .iter()
            .map(|&m| 700.0 * (10.0f32.powf(m / 2595.0) - 1.0))
            .collect();
        let bins: Vec<usize> = hz_points
            .iter()
            .map(|&hz| ((n_fft as f32 + 1.0) * hz / sr as f32).floor() as usize)
            .collect();

        let mut filters = Array2::<f32>::zeros((n_mels, n_freqs));
        for m in 1..=n_mels {
            let (f_left, f_center, f_right) = (bins[m - 1], bins[m], bins[m + 1]);
            for k in f_left..f_center {
                if k < n_freqs && f_center > f_left {
                    filters[[m - 1, k]] = (k - f_left) as f32 / (f_center - f_left) as f32;
                }
            }
            for k in f_center..f_right {
                if k < n_freqs && f_right > f_center {
                    filters[[m - 1, k]] = (f_right - k) as f32 / (f_right - f_center) as f32;
                }
            }
        }
        filters
    }

    fn compute_log_mel(&self, samples: &[f32]) -> Array2<f32> {
        let win_len = 400; // 25ms @ 16kHz
        let hop_len = 160; // 10ms @ 16kHz
        let n_fft = 400;
        let n_freqs = n_fft / 2 + 1;

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(n_fft);

        // Hamming window
        let window: Vec<f32> = (0..win_len)
            .map(|i| 0.54 - 0.46 * (2.0 * PI * i as f32 / (win_len - 1) as f32).cos())
            .collect();

        let num_frames = if samples.len() >= win_len {
            (samples.len() - win_len) / hop_len + 1
        } else {
            0
        };

        let mut power_spec = Array2::<f32>::zeros((num_frames, n_freqs));
        let mut buffer = vec![Complex::new(0.0, 0.0); n_fft];

        for frame_idx in 0..num_frames {
            let start = frame_idx * hop_len;
            for i in 0..win_len {
                // Pre-emphasis (0.97) + window
                let s = if start + i > 0 {
                    samples[start + i] - 0.97 * samples[start + i - 1]
                } else {
                    samples[start + i]
                };
                buffer[i] = Complex::new(s * window[i], 0.0);
            }
            fft.process(&mut buffer);

            for k in 0..n_freqs {
                power_spec[[frame_idx, k]] = buffer[k].norm_sqr();
            }
        }

        // Apply Mel filterbank + Log
        let mut mel_spec = power_spec.dot(&self.mel_filters.t());
        mel_spec.mapv_inplace(|v| (v.max(1e-10)).ln());
        mel_spec
    }

    /// Low Frame Rate (LFR): m=7 frames stack, n=6 stride
    fn apply_lfr(mel: &Array2<f32>, m: usize, n: usize) -> Array2<f32> {
        let t = mel.nrows();
        let d = mel.ncols();
        if t == 0 {
            return Array2::zeros((0, d * m));
        }
        let t_lfr = (t as f32 / n as f32).ceil() as usize;
        let pad_left = (m - 1) / 2;

        let mut out = Array2::<f32>::zeros((t_lfr, d * m));
        for i in 0..t_lfr {
            let center = i * n;
            for j in 0..m {
                let src_idx = (center + j).saturating_sub(pad_left).min(t - 1);
                for k in 0..d {
                    out[[i, j * d + k]] = mel[[src_idx, k]];
                }
            }
        }
        out
    }
}