use crate::config::{ClickButton, ClickType, ClickerConfig, DelayMode};
use crate::constants::*;
use crossbeam::channel::{self, Receiver, Sender};
use enigo::{Button, Direction, Enigo, Mouse, Settings};
use rand::RngCore;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub enum TimerCommand {
    Stop,
}

#[derive(Debug, Clone)]
pub enum StatusUpdate {
    Error(String),
}

pub struct PrecisionTimer {
    timer_handle: Option<JoinHandle<()>>,
    status_sender: Option<Sender<StatusUpdate>>,
    command_sender: Option<Sender<TimerCommand>>,
    is_running: Arc<AtomicBool>,
}

impl PrecisionTimer {
    pub fn new() -> Self {
        Self {
            timer_handle: None,
            status_sender: None,
            command_sender: None,
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(&mut self, config: ClickerConfig) -> Receiver<StatusUpdate> {
        self.stop();

        let (status_tx, status_rx) = channel::unbounded();
        let (cmd_tx, cmd_rx) = channel::unbounded();

        self.status_sender = Some(status_tx.clone());
        self.command_sender = Some(cmd_tx);

        let is_running = self.is_running.clone();
        is_running.store(true, Ordering::SeqCst);

        let handle = thread::spawn(move || {
            Self::timer_loop(config, status_tx, cmd_rx, is_running);
        });

        self.timer_handle = Some(handle);
        status_rx
    }

    pub fn stop(&mut self) {
        // Signal the thread to stop
        self.is_running.store(false, Ordering::SeqCst);

        if let Some(sender) = &self.command_sender {
            let _ = sender.send(TimerCommand::Stop);
        }

        // Don't block the UI thread - let the thread finish naturally
        self.status_sender = None;
        self.command_sender = None;
    }

    fn timer_loop(
        config: ClickerConfig,
        status_sender: Sender<StatusUpdate>,
        command_receiver: Receiver<TimerCommand>,
        is_running: Arc<AtomicBool>,
    ) {
        // Create Enigo ONCE at the start of the timer session
        let mut enigo = match Enigo::new(&Settings::default()) {
            Ok(enigo) => enigo,
            Err(e) => {
                let _ =
                    status_sender.send(StatusUpdate::Error(format!("{}: {}", ERROR_ENIGO_INIT, e)));
                is_running.store(false, Ordering::SeqCst);
                return;
            }
        };

        // Create RNG ONCE at the start (for jitter mode)
        let mut rng = rand::thread_rng();

        // Pre-calculate CPS interval ONCE at the start (for CPS mode)
        let cps_interval = if config.cps > 0.0 {
            Duration::from_nanos((1_000_000_000.0 / config.cps) as u64)
        } else {
            Duration::from_millis(config.interval_ms)
        };

        let mut next_target_time = Instant::now();

        loop {
            // Check for stop command
            if let Ok(TimerCommand::Stop) = command_receiver.try_recv() {
                break;
            }

            if !is_running.load(Ordering::SeqCst) {
                break;
            }

            let now = Instant::now();
            if now < next_target_time {
                let sleep_duration = next_target_time - now;
                // Sleep the thread until the next target time
                thread::sleep(sleep_duration);
                continue;
            }

            // Determine delay until next tick based on mode
            let delay_duration = match config.delay_mode {
                DelayMode::Cps => cps_interval,
                DelayMode::Jitter => {
                    let min_ms = config.min_delay_ms;
                    let max_ms = config.max_delay_ms;

                    // Guard against inverted or equal ranges
                    if max_ms <= min_ms {
                        Duration::from_millis(min_ms)
                    } else {
                        // Sample a uniform integer in [min_ms, max_ms] using RNG's next_u64()
                        let range = max_ms - min_ms + 1;
                        // Use modulus sampling; range should be small (ms), so bias is negligible here.
                        let r = rng.next_u64() % range;
                        let ms = min_ms + r;
                        Duration::from_millis(ms)
                    }
                }
            };

            // Perform the click action according to config using enigo's Mouse trait methods
            match config.click_type {
                ClickType::Single => {
                    match config.click_button {
                        ClickButton::Left => {
                            let _ = enigo.button(Button::Left, Direction::Press);
                            let _ = enigo.button(Button::Left, Direction::Release);
                        }
                        ClickButton::Right => {
                            let _ = enigo.button(Button::Right, Direction::Press);
                            let _ = enigo.button(Button::Right, Direction::Release);
                        }
                        ClickButton::Middle => {
                            let _ = enigo.button(Button::Middle, Direction::Press);
                            let _ = enigo.button(Button::Middle, Direction::Release);
                        }
                    }
                }
                ClickType::Double => {
                    match config.click_button {
                        ClickButton::Left => {
                            let _ = enigo.button(Button::Left, Direction::Press);
                            let _ = enigo.button(Button::Left, Direction::Release);
                            thread::sleep(Duration::from_millis(40));
                            let _ = enigo.button(Button::Left, Direction::Press);
                            let _ = enigo.button(Button::Left, Direction::Release);
                        }
                        ClickButton::Right => {
                            let _ = enigo.button(Button::Right, Direction::Press);
                            let _ = enigo.button(Button::Right, Direction::Release);
                            thread::sleep(Duration::from_millis(40));
                            let _ = enigo.button(Button::Right, Direction::Press);
                            let _ = enigo.button(Button::Right, Direction::Release);
                        }
                        ClickButton::Middle => {
                            let _ = enigo.button(Button::Middle, Direction::Press);
                            let _ = enigo.button(Button::Middle, Direction::Release);
                            thread::sleep(Duration::from_millis(40));
                            let _ = enigo.button(Button::Middle, Direction::Press);
                            let _ = enigo.button(Button::Middle, Direction::Release);
                        }
                    }
                }
                ClickType::Hold => {
                    match config.click_button {
                        ClickButton::Left => {
                            let _ = enigo.button(Button::Left, Direction::Press);
                            thread::sleep(Duration::from_millis(50));
                            let _ = enigo.button(Button::Left, Direction::Release);
                        }
                        ClickButton::Right => {
                            let _ = enigo.button(Button::Right, Direction::Press);
                            thread::sleep(Duration::from_millis(50));
                            let _ = enigo.button(Button::Right, Direction::Release);
                        }
                        ClickButton::Middle => {
                            let _ = enigo.button(Button::Middle, Direction::Press);
                            thread::sleep(Duration::from_millis(50));
                            let _ = enigo.button(Button::Middle, Direction::Release);
                        }
                    }
                }
            }

            // Advance the next target time
            next_target_time = Instant::now() + delay_duration;
        }
    }
}
