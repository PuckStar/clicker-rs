use crate::config::{ClickButton, ClickType, ClickerConfig, DelayMode};
use crate::constants::*;
use crossbeam::channel::{self, Receiver, Sender};
use enigo::{Button, Direction, Enigo, Mouse, Settings};
use rand::RngCore;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

fn random_point_in_box(config: &ClickerConfig, rng: &mut impl rand::Rng) -> (i32, i32) {
    if !config.bounding_box_enabled {
        return (0, 0);
    }

    let x1 = config.box_x1;
    let y1 = config.box_y1;
    let x2 = config.box_x2;
    let y2 = config.box_y2;

    let x = rng.random_range(x1.min(x2)..=x1.max(x2));
    let y = rng.random_range(y1.min(y2)..=y1.max(y2));
    (x, y)
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
        self.is_running.store(false, Ordering::SeqCst);

        if let Some(sender) = &self.command_sender {
            let _ = sender.send(TimerCommand::Stop);
        }

        self.status_sender = None;
        self.command_sender = None;
    }

    fn timer_loop(
        config: ClickerConfig,
        status_sender: Sender<StatusUpdate>,
        command_receiver: Receiver<TimerCommand>,
        is_running: Arc<AtomicBool>,
    ) {
        let mut enigo = match Enigo::new(&Settings::default()) {
            Ok(enigo) => enigo,
            Err(e) => {
                let _ =
                    status_sender.send(StatusUpdate::Error(format!("{}: {}", ERROR_ENIGO_INIT, e)));
                is_running.store(false, Ordering::SeqCst);
                return;
            }
        };

        let mut rng = rand::rng();

        let cps_interval = if config.cps > 0.0 {
            Duration::from_nanos((1_000_000_000.0 / config.cps) as u64)
        } else {
            Duration::from_millis(config.interval_ms)
        };

        let mut next_target_time = Instant::now();

        loop {
            if let Ok(TimerCommand::Stop) = command_receiver.try_recv() {
                break;
            }

            if !is_running.load(Ordering::SeqCst) {
                break;
            }

            let now = Instant::now();
            if now < next_target_time {
                let sleep_duration = next_target_time - now;
                thread::sleep(sleep_duration);
                continue;
            }

            let delay_duration = match config.delay_mode {
                DelayMode::Cps => cps_interval,
                DelayMode::Jitter => {
                    let min_ms = config.min_delay_ms;
                    let max_ms = config.max_delay_ms;
                    if max_ms <= min_ms {
                        Duration::from_millis(min_ms)
                    } else {
                        let range = max_ms - min_ms + 1;
                        let r = rng.next_u64() % range;
                        let ms = min_ms + r;
                        Duration::from_millis(ms)
                    }
                }
            };

            if config.bounding_box_enabled {
                let (mx, my) = random_point_in_box(&config, &mut rng);
                enigo.mouse_move_to(mx, my);
            }

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

            next_target_time = Instant::now() + delay_duration;
        }
    }
}
