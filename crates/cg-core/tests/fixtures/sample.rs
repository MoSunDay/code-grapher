use std::fmt;

pub const MAX: i32 = 10;
static NAME: &str = "x";
pub type Alias = Vec<u8>;

pub struct Point {
    pub x: i32,
    y: i32,
}

pub enum Shape {
    Circle,
    Square,
}

pub trait Drawable {
    fn draw(&self);
    fn desc(&self) -> String {
        "d".to_string()
    }
}

macro_rules! square {
    ($x:expr) => {
        $x * $x
    };
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

impl Point {
    pub fn new(x: i32, y: i32) -> Self {
        Point { x, y }
    }

    fn norm(&self) -> i32 {
        let handler = |a: i32| a + 1;
        handler(self.x)
    }

    fn distance(&self, other: &Point) -> i32 {
        abs_diff(self.x, other.x)
    }
}

fn abs_diff(a: i32, b: i32) -> i32 {
    if a > b {
        a - b
    } else {
        b - a
    }
}

mod inner_mod {
    pub fn hidden() -> i32 {
        42
    }
}

pub fn main() {
    let p = Point::new(1, 2);
    let q = p.clone();
    let v = vec![1, 2];
    apply(abs_diff);
    let add = |x: i32, y: i32| x + y;
    square!(p.x);
    format_args!("{}", 1);
    run_all(&abs_diff);
    let _ = add(1, 2);
}

fn apply(f: fn(i32, i32) -> i32) {
    let _ = f(1, 2);
}

fn run_all(f: fn(i32, i32) -> i32) {
    let _ = f(3, 4);
}

fn nested() {
    fn inner() -> i32 {
        42
    }
    inner();
}
