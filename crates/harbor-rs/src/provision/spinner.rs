//! Ticking spinner with continuously updating elapsed time.
//!
//! Starts when constructed, ticks every 100ms via a background tokio task,
//! and stops when `success()`/`fail()` is called or on drop.
//!
//! In debug mode, becomes a no-op — output streams directly instead.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use console::{Style, Term};
use tokio::task::JoinHandle;

static SPINNER_CHARS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const TICK_INTERVAL: Duration = Duration::from_millis(100);

/// Fallback width when the terminal size cannot be queried (non-TTY, pipe).
const FALLBACK_COLS: usize = 80;

/// Overhead chars reserved in the spinner line: spinner glyph + space +
/// space before timer + "(" + timer + ")". Timer tops out at ~10 chars
/// (`99m 59s`), so we reserve 14 to keep the arithmetic cheap and leave
/// one column of slack so the cursor never lands on the right edge.
const RENDER_OVERHEAD: usize = 14;

/// A ticking spinner with status text that updates in place.
pub struct Spinner {
    inner: Option<SpinnerInner>,
}

struct SpinnerInner {
    state: Arc<Mutex<SpinnerState>>,
    task: JoinHandle<()>,
}

struct SpinnerState {
    start: Instant,
    step: String,
    idx: usize,
    active: bool,
}

impl Spinner {
    /// Start a spinner with the given initial status text.
    /// In debug mode, this becomes a no-op and the initial step is printed once.
    pub fn start(initial_step: impl Into<String>, debug: bool) -> Self {
        let step = initial_step.into();

        if debug {
            eprintln!("→ {step}");
            return Self { inner: None };
        }

        let state = Arc::new(Mutex::new(SpinnerState {
            start: Instant::now(),
            step,
            idx: 0,
            active: true,
        }));

        let task_state = Arc::clone(&state);
        let task = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(TICK_INTERVAL);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                let mut s = task_state.lock().expect("spinner state mutex poisoned");
                if !s.active {
                    break;
                }
                render(&s);
                s.idx = s.idx.wrapping_add(1);
            }
        });

        Self {
            inner: Some(SpinnerInner { state, task }),
        }
    }

    /// Update the status text. Timer keeps ticking.
    pub fn set_step(&self, step: impl Into<String>) {
        let step = step.into();
        if let Some(ref inner) = self.inner {
            let mut s = inner.state.lock().expect("spinner state mutex poisoned");
            s.step = step;
        } else {
            eprintln!("→ {step}");
        }
    }

    /// Stop ticking and print a success line with total elapsed time.
    pub fn success(mut self, msg: impl Into<String>) {
        let msg = msg.into();
        if let Some(inner) = self.inner.take() {
            let elapsed = {
                let mut s = inner.state.lock().expect("spinner state mutex poisoned");
                s.active = false;
                s.start.elapsed()
            };
            inner.task.abort();

            let green = Style::new().green().bold();
            let dim = Style::new().dim();
            eprint!("\r\x1b[K");
            eprintln!(
                "{} {} {}",
                green.apply_to("✓"),
                msg,
                dim.apply_to(format!("({})", format_duration(elapsed)))
            );
        } else {
            eprintln!("✓ {msg}");
        }
    }

    /// Stop ticking and print a failure line (leaves error reporting to caller).
    pub fn fail(mut self) {
        if let Some(inner) = self.inner.take() {
            inner
                .state
                .lock()
                .expect("spinner state mutex poisoned")
                .active = false;
            inner.task.abort();
            eprint!("\r\x1b[K");
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            inner
                .state
                .lock()
                .expect("spinner state mutex poisoned")
                .active = false;
            inner.task.abort();
            eprint!("\r\x1b[K");
        }
    }
}

fn render(state: &SpinnerState) {
    let idx = state.idx % SPINNER_CHARS.len();
    let spinner_char = SPINNER_CHARS.get(idx).copied().unwrap_or('⠋');
    let time_str = format_duration(state.start.elapsed());

    let cyan = Style::new().cyan();
    let dim = Style::new().dim();

    let cols = Term::stderr()
        .size_checked()
        .map_or(FALLBACK_COLS, |(_rows, c)| c as usize);
    let budget = cols.saturating_sub(RENDER_OVERHEAD + time_str.chars().count());
    let step = truncate(&state.step, budget);

    eprint!("\r\x1b[K");
    eprint!(
        "{} {} {}",
        cyan.apply_to(spinner_char),
        step,
        dim.apply_to(format!("({time_str})"))
    );
}

/// Truncate `s` to at most `max` chars, appending an ellipsis if cut.
/// Operates on chars, not bytes, so multibyte glyphs in status text
/// (paths, repo names) don't split in the middle of a codepoint.
fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let char_count = s.chars().count();
    if char_count <= max {
        return s.to_owned();
    }
    if max == 1 {
        return "…".to_owned();
    }
    let keep: String = s.chars().take(max - 1).collect();
    format!("{keep}…")
}

fn format_duration(d: Duration) -> String {
    let total = d.as_secs();
    let mins = total / 60;
    let secs = total % 60;
    if mins > 0 {
        format!("{mins}m {secs:02}s")
    } else {
        format!("{secs}s")
    }
}
