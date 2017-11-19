extern crate image;
extern crate rand;
extern crate rayon;

use std::env;

use image::{Pixel, ImageBuffer, Rgb, Rgba};
use rand::{Rng, random};
use rayon::prelude::*;

type Image = ImageBuffer<Rgba<u8>, Vec<u8>>;

#[derive(Debug, Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
}

#[derive(Debug, Clone, Copy)]
struct Triangle {
    a: Point,
    b: Point,
    c: Point,
}

impl Triangle {
    fn random(mut width: i32, mut height: i32) -> Self {
        const PAD: i32 = 10;
        let mut rng = rand::thread_rng();
        let a = Point {
            x: rng.gen_range(0, width - PAD),
            y: rng.gen_range(0, height),
        };
        let b = Point {
            x: rng.gen_range(a.x + PAD, width),
            y: rng.gen_range(0, height),
        };
        let c = Point {
            x: rng.gen_range(a.x, b.x),
            y: rng.gen_range(b.y, height),
        };
        Self { a, b, c }
    }

    fn contains(&self, point: Point) -> bool {
        fn orient2d(a: Point, b: Point, c: Point) -> i32 {
            (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
        }
        let w0 = orient2d(self.a, self.b, point);
        let w1 = orient2d(self.b, self.c, point);
        let w2 = orient2d(self.c, self.a, point);
        w0 >= 0 && w1 >= 0 && w2 >= 0
    }

    fn bounding(&self, w: i32, h: i32) -> (i32, i32, i32, i32) {
        use std::cmp::{min, max};
        let min_x = min(min(self.a.x, self.b.x), self.c.x);
        let min_y = min(min(self.a.y, self.b.y), self.c.y);
        let max_x = max(max(self.a.x, self.b.x), self.c.x);
        let max_y = max(max(self.a.y, self.b.y), self.c.y);
        (max(min_x, 0), max(min_y, 0), min(max_x, w), min(max_y, h))
    }
}

fn rmse(a: &Image, b: &Image) -> f32 {
    let mut error = 0.0;
    for (&a, &b) in a.iter().zip(b.iter()) {
        error += (a as f32 - b as f32).abs().powi(2);
    }
    let n = a.height() * a.width();
    error /= n as f32;
    error
}

fn main() {
    let filename = env::args().nth(1).expect("Usage: nail <filename>");
    let image: Image = image::open(&filename).unwrap().to_rgba();
    let avg_color = {
        let mut b = [0usize; 3];
        for (_x, _y, p) in image.enumerate_pixels() {
            b[0] += p.data[0] as usize;
            b[1] += p.data[1] as usize;
            b[2] += p.data[2] as usize;
        }
        let n = (image.width() * image.height()) as usize;
        b[0] /= n;
        b[1] /= n;
        b[2] /= n;
        *Rgba::from_slice(&[b[0] as u8, b[1] as u8, b[2] as u8, 255])
    };
    let (w, h) = image.dimensions();
    let mut buffer: Image = image::ImageBuffer::new(w, h);
    for (_x, _y, p) in buffer.enumerate_pixels_mut() {
        *p = avg_color;
    }

    let _ = buffer.save(&"output.png");
    let w = w as i32;
    let h = h as i32;

    const N_ITERS: usize = 200;
    const N_TRIANGLES: usize = 50;

    for iter in 0..N_TRIANGLES {
        use std::sync::Mutex;
        let best = Mutex::new((::std::f32::INFINITY, None, None));

        (0..N_ITERS).into_par_iter().map(|_| {
            let mut buffer = buffer.clone();
            let triangle = Triangle::random(w, h);
            let (x0, y0, x1, y1) = triangle.bounding(w, h);

            let mut pixels = Vec::new();
            let mut avg_pixel = [0, 0, 0];
            for y in y0..y1 {
                for x in x0..x1 {
                    if triangle.contains(Point { x, y }) {
                        let p = image.get_pixel(x as u32, y as u32).channels();
                        avg_pixel[0] += p[0] as usize;
                        avg_pixel[1] += p[1] as usize;
                        avg_pixel[2] += p[2] as usize;
                        pixels.push((x, y));
                    }
                }
            }
            if pixels.len() == 0 {
                return;
            }
            avg_pixel[0] /= pixels.len();
            avg_pixel[1] /= pixels.len();
            avg_pixel[2] /= pixels.len();
            for &(x, y) in &pixels {
                let c = *Rgba::from_slice(
                    &[
                        avg_pixel[0] as u8,
                        avg_pixel[1] as u8,
                        avg_pixel[2] as u8,
                        160,
                    ],
                );
                buffer.get_pixel_mut(x as u32, y as u32).blend(&c);
            }

            let score = rmse(&image, &buffer);
            let mut handle = best.lock().unwrap();
            if score < handle.0 {
                handle.0 = score;
                handle.1 = Some(triangle);
                handle.2 = Some(buffer);
            }
        }).count();
        let mut h = best.lock().unwrap();
        buffer = h.2.take().unwrap();
    }
    buffer.save(&"output2.png");
}
