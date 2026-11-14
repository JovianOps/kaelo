//! Game-loop embedding: the host owns the loop and the renderer, the script owns
//! the rules. `cargo run --example game_loop`

use kaelo::{Engine, Value};
use std::cell::RefCell;
use std::rc::Rc;

struct Screen {
    pixels: Vec<char>,
    w: usize,
    h: usize,
}

impl Screen {
    fn new(w: usize, h: usize) -> Self {
        Screen { pixels: vec![' '; w * h], w, h }
    }
    fn clear(&mut self) {
        for p in self.pixels.iter_mut() {
            *p = ' ';
        }
    }
    fn rect(&mut self, x: i64, y: i64, w: i64, h: i64, ch: char) {
        for yy in y..(y + h) {
            for xx in x..(x + w) {
                if xx >= 0 && yy >= 0 && (xx as usize) < self.w && (yy as usize) < self.h {
                    self.pixels[yy as usize * self.w + xx as usize] = ch;
                }
            }
        }
    }
    fn show(&self) {
        for row in self.pixels.chunks(self.w) {
            println!("{}", row.iter().collect::<String>());
        }
    }
}

fn main() {
    let screen = Rc::new(RefCell::new(Screen::new(40, 12)));

    let mut k = Engine::new();

    // host-provided draw calls — the script never touches memory directly
    let s = screen.clone();
    k.host_fn("clear", Box::new(move |_, _| {
        s.borrow_mut().clear();
        Ok(Value::None)
    }));
    let s = screen.clone();
    k.host_fn("rect", Box::new(move |_, args| {
        let (x, y, w, h) = match args {
            [Value::Int(x), Value::Int(y), Value::Int(w), Value::Int(h)] => (*x, *y, *w, *h),
            _ => return Err(kaelo::RtError::new("rect(x, y, w, h)")),
        };
        s.borrow_mut().rect(x, y, w, h, '#');
        Ok(Value::None)
    }));

    // a scripted game: a bar that slides back and forth, 60 frames per second
    k.run(
        r#"
var x = 0;
var dir = 1;
var frame = 0;

fn update(dt) {
    frame = frame + 1;
    x = x + dir;
    if x > 30 {
        dir = 0 - 1;
    }
    if x < 0 {
        dir = 1;
    }
}

fn draw() {
    clear();
    rect(x, 5, 8, 2);
    rect(0, 0, 1, 12);
}
"#,
    )
    .expect("script should load");

    let started = std::time::Instant::now();
    let frames = 600; // 10 seconds of gameplay logic at 60 Hz
    for _ in 0..frames {
        k.call("update", &[Value::from(1.0 / 60.0)]).expect("update");
        k.call("draw", &[]).expect("draw");
    }
    let elapsed = started.elapsed().as_secs_f64();

    screen.borrow().show();
    println!(
        "{} frames of script logic in {:.1} ms  ({:.0} fps equivalent, no GC pauses)",
        frames,
        elapsed * 1000.0,
        frames as f64 / elapsed
    );
}