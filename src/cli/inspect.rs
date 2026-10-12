use std::io::{self, Write, stdout};
use std::time::{Duration, Instant};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

use super::{InspectArgs, open_keyboard, parse_key_list};
use crate::keyboard::{AnalogReading, plan_queries};

pub fn cmd_inspect(args: InspectArgs) -> Result<(), Box<dyn std::error::Error>> {
    let mut keyboard = open_keyboard(args.device.as_ref())?;
    let key_list = parse_key_list(&args.keys)?;
    if key_list.is_empty() {
        return Err("No valid keys specified for monitoring".into());
    }

    let queries = plan_queries(&key_list);

    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, cursor::Hide)?;

    let frame_interval = if args.rate > 0 {
        Duration::from_secs_f64(1.0 / (args.rate as f64))
    } else {
        Duration::from_millis(16)
    };

    let mut last_poll = Instant::now();
    let mut poll_count = 0u64;
    let mut current_hz = 0.0f64;
    let mut hz_timer = Instant::now();

    let res = (|| -> Result<(), Box<dyn std::error::Error>> {
        loop {
            if event::poll(Duration::from_millis(0))?
                && let Event::Key(key_event) = event::read()?
                && (key_event.code == KeyCode::Char('q')
                    || key_event.code == KeyCode::Esc
                    || (key_event.code == KeyCode::Char('c')
                        && key_event.modifiers.contains(KeyModifiers::CONTROL)))
            {
                break;
            }

            let readings = keyboard.read_keys(&queries)?;
            poll_count += 1;

            if hz_timer.elapsed() >= Duration::from_millis(500) {
                current_hz = (poll_count as f64) / hz_timer.elapsed().as_secs_f64();
                poll_count = 0;
                hz_timer = Instant::now();
            }

            execute!(out, cursor::MoveTo(0, 0))?;
            execute!(
                out,
                SetForegroundColor(Color::Cyan),
                Print("Madlion Analog Switch Monitor\r\n"),
                ResetColor,
                Print(format!(
                    "Device: {} | Rate: {:.1} Hz | Queries per poll: {}\r\n",
                    keyboard.path().display(),
                    current_hz,
                    queries.len()
                )),
                Print("Press 'q' or Esc to exit\r\n"),
                Print("----------------------------------------------------------------------\r\n")
            )?;

            for reading in &readings {
                render_key_bar(&mut out, reading)?;
            }

            out.flush()?;

            let elapsed = last_poll.elapsed();
            std::thread::sleep(frame_interval.saturating_sub(elapsed));
            last_poll = Instant::now();
        }
        Ok(())
    })();

    execute!(out, cursor::Show, LeaveAlternateScreen)?;
    disable_raw_mode()?;
    res
}

fn render_key_bar(out: &mut io::Stdout, reading: &AnalogReading) -> io::Result<()> {
    let bar_width = 32;
    let filled_len = ((reading.normalized * (bar_width as f32)).round() as usize).min(bar_width);
    let empty_len = bar_width.saturating_sub(filled_len);

    let bar_color = if reading.normalized > 0.75 {
        Color::Red
    } else if reading.normalized > 0.35 {
        Color::Yellow
    } else if reading.normalized > 0.05 {
        Color::Green
    } else {
        Color::DarkGrey
    };

    let key_name = format!("{}", reading.key);
    execute!(
        out,
        SetForegroundColor(Color::White),
        Print(format!(
            "{:<8} [idx {:02}] ",
            key_name, reading.matrix_index
        )),
        ResetColor,
        Print("["),
        SetForegroundColor(bar_color),
        Print("=".repeat(filled_len)),
        ResetColor,
        Print(" ".repeat(empty_len)),
        Print("] "),
        SetForegroundColor(Color::Cyan),
        Print(format!("{:4.2} mm ", reading.depth_mm)),
        SetForegroundColor(Color::Magenta),
        Print(format!("({:5.1}%)\r\n", reading.normalized * 100.0)),
        ResetColor
    )?;

    Ok(())
}
