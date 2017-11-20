extern crate image;
extern crate rand;
extern crate rayon;

use std::env;
use std::path::Path;
use std::fs::File;
use std::io::Write;

use image::{Pixel, ImageBuffer, Rgba};
use rand::Rng;
use rayon::prelude::*;


const NUM_TRIANGLES: usize = 10;
const TRANSPARENCY: u8 = 230;
const N_ITERS: usize = 10_000;
const DOWNSCALE: u32 = 64;


type Image = ImageBuffer<Rgba<u8>, Vec<u8>>;
type Color = [u8; 4];

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

#[derive(Debug, Clone, Copy)]
struct ColorTriangle {
    triangle: Triangle,
    color: Color,
}

impl std::ops::Deref for ColorTriangle {
    type Target = Triangle;
    fn deref(&self) -> &Self::Target {
        &self.triangle
    }
}

impl Triangle {
    fn random(width: u32, height: u32) -> Self {
        const PAD: i32 = 1;

        let width = width as i32;
        let height = height as i32;
        let hw = width / 5;
        let hh = height / 5;

        let mut rng = rand::thread_rng();
        let a = Point {
            x: rng.gen_range(-hw, width - PAD),
            y: rng.gen_range(-hh, height + hw),
        };
        let b = Point {
            x: rng.gen_range(a.x + PAD, width + hw),
            y: rng.gen_range(-hh, height - PAD),
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

    fn bounding(&self, w: u32, h: u32) -> (i32, i32, i32, i32) {
        use std::cmp::{min, max};
        let min_x = min(min(self.a.x, self.b.x), self.c.x);
        let min_y = min(min(self.a.y, self.b.y), self.c.y);
        let max_x = max(max(max(self.a.x, self.b.x), self.c.x), 0);
        let max_y = max(max(max(self.a.y, self.b.y), self.c.y), 0);
        (
            max(min_x, 0),
            max(min_y, 0),
            min(max_x, w as i32),
            min(max_y, h as i32),
        )
    }
}

#[derive(Debug)]
enum Error {
    MissingInput,
    TriangulationFailed,
    IoError(::std::io::Error),
    ImageError(image::ImageError),
}

struct Svg {
    background: Color,
    triangles: Vec<ColorTriangle>,
    width: u32,
    height: u32,
}

impl Svg {
    fn save(&self, filename: &AsRef<Path>) -> Result<(), ::std::io::Error> {
        fn color_to_hex(c: Color) -> String {
            format!("#{:x}{:x}{:x}", c[0], c[1], c[2])
        }

        let mut s = String::new();
        s.push_str(&format!(
            r##"<?xml version="1.0" standalone="no"?>
    <svg viewBox = "0 0 {} {}" version = "1.1" xmlns="http://www.w3.org/2000/svg">
        <rect x="0" y="0" width="{0}" height="{1}" fill="{}"/>{}"##,
            self.width,
            self.height,
            color_to_hex(self.background),
            '\n'
        ));
        for triangle in &self.triangles {
            s.push_str(&format!(
                "    <polygon points=\"{} {}, {} {}, {} {}\" fill=\"{}\" fill-opacity=\"{}\" />\n",
                triangle.a.x,
                triangle.a.y,
                triangle.b.x,
                triangle.b.y,
                triangle.c.x,
                triangle.c.y,
                color_to_hex(triangle.color),
                triangle.color[3] as f32 / 255.0
            ));
        }
        s.push_str("</svg>\n");
        let mut f = File::create(filename).unwrap();
        f.write_all(s.as_bytes())
    }

    fn scale(&mut self, sx: f32, sy: f32) {
        for triangle in self.triangles.iter_mut() {
            triangle.triangle.a.x = (triangle.triangle.a.x as f32 * sx) as i32;
            triangle.triangle.b.x = (triangle.triangle.b.x as f32 * sx) as i32;
            triangle.triangle.c.x = (triangle.triangle.c.x as f32 * sx) as i32;

            triangle.triangle.a.y = (triangle.triangle.a.y as f32 * sy) as i32;
            triangle.triangle.b.y = (triangle.triangle.b.y as f32 * sy) as i32;
            triangle.triangle.c.y = (triangle.triangle.c.y as f32 * sy) as i32;
        }
    }
}

/// Compute the next triangle for the image.
fn next_triangle(target_image: &Image, current_image: &Image) -> Option<ColorTriangle> {
    (0..N_ITERS)
        .into_par_iter()
        .flat_map(|_i| {
            let (w, h) = target_image.dimensions();
            let triangle = Triangle::random(w, h);
            let (x0, y0, x1, y1) = triangle.bounding(w, h);

            let cap = (y1 - y0) as usize + (x1 - x0) as usize;
            if cap > 10_000 {
                println!("cap={}", cap);
                println!("{:?}", triangle.bounding(w, h));
                println!("w={} h={}", w, h);
                println!("triangle: {:?}", triangle);
            }
            let mut pixels = Vec::with_capacity(cap);
            let mut avg_pixel = [0, 0, 0];
            for y in y0..y1 {
                for x in x0..x1 {
                    if triangle.contains(Point { x, y }) {
                        let p = target_image.get_pixel(x as u32, y as u32).channels();
                        avg_pixel[0] += p[0] as usize;
                        avg_pixel[1] += p[1] as usize;
                        avg_pixel[2] += p[2] as usize;
                        pixels.push((x as u32, y as u32));
                    }
                }
            }
            if pixels.len() == 0 {
                return None;
            }
            avg_pixel[0] /= pixels.len();
            avg_pixel[1] /= pixels.len();
            avg_pixel[2] /= pixels.len();
            let color = [
                avg_pixel[0] as u8,
                avg_pixel[1] as u8,
                avg_pixel[2] as u8,
                TRANSPARENCY,
            ];
            let score = {
                let mut s = 0isize;
                let c = *Rgba::from_slice(&color);
                for &(x, y) in &pixels {
                    let target = target_image.get_pixel(x, y);
                    let before = *current_image.get_pixel(x, y);
                    let old_error = {
                        (target[0] as i16 - before[0] as i16).pow(2) as isize +
                            (target[1] as i16 - before[1] as i16).pow(2) as isize +
                            (target[2] as i16 - before[2] as i16).pow(2) as isize
                    };
                    let after = {
                        let mut a = before;
                        a.blend(&c);
                        a
                    };
                    let new_error = {
                        (target[0] as i16 - after[0] as i16).pow(2) as isize +
                            (target[1] as i16 - after[1] as i16).pow(2) as isize +
                            (target[2] as i16 - after[2] as i16).pow(2) as isize
                    };
                    s += new_error - old_error;
                }
                s // / pixels.len() as isize
            };
            Some((score, ColorTriangle { triangle, color }))
        })
        .min_by_key(|&(score, _)| score)
        .map(|(_s, triangle)| triangle)
}

fn triangulate(image: Image) -> Result<Svg, Error> {
    fn avg_color(img: &Image) -> Color {
        let n = {
            let (w, h) = img.dimensions();
            (w * h) as usize
        };
        let mut c = [0; 4];
        for (_x, _y, p) in img.enumerate_pixels() {
            c[0] += p.data[0] as usize;
            c[1] += p.data[1] as usize;
            c[2] += p.data[2] as usize;
        }
        [(c[0] / n) as u8, (c[1] / n) as u8, (c[2] / n) as u8, 255]
    }

    fn fill_with(img: &mut Image, color: Color) {
        let c = *Rgba::from_slice(&color);
        for (_x, _y, p) in img.enumerate_pixels_mut() {
            *p = c;
        }
    }

    fn rasterize_triangle(image: &mut Image, triangle: ColorTriangle) {
        let (w, h) = image.dimensions();
        let (x0, y0, x1, y1) = triangle.bounding(w, h);

        let color = Rgba::from_slice(&triangle.color);
        for y in y0..y1 {
            for x in x0..x1 {
                if triangle.contains(Point { x, y }) {
                    let mut p = image.get_pixel_mut(x as u32, y as u32);
                    p.blend(color);
                }
            }
        }
    }

    let (w, h) = image.dimensions();
    let downsampled =
        image::imageops::resize(&image, DOWNSCALE, DOWNSCALE, image::FilterType::Nearest);

    let mut buffer = image.clone();
    let background_color = avg_color(&buffer);
    fill_with(&mut buffer, background_color);

    let mut svg = Svg {
        background: background_color,
        triangles: vec![],
        width: image.width(),
        height: image.height(),
    };

    for _ in 0..NUM_TRIANGLES {
        let triangle = next_triangle(&downsampled, &buffer).ok_or(
            Error::TriangulationFailed,
        )?;
        rasterize_triangle(&mut buffer, triangle);
        svg.triangles.push(triangle);
    }

    let scale_x = w as f32 / DOWNSCALE as f32;
    let scale_y = h as f32 / DOWNSCALE as f32;
    svg.scale(scale_x, scale_y);

    Ok(svg)
}

fn do_stuff() -> Result<(), Error> {
    let filename = env::args().nth(1).ok_or(Error::MissingInput)?;
    let image: Image = image::open(&filename)
        .map_err(|e| Error::ImageError(e))?
        .to_rgba();
    let triangulated = triangulate(image)?;
    triangulated
        .save(&format!("out-{}.svg", filename))
        .map_err(|e| Error::IoError(e))?;
    Ok(())
}

fn main() {
    match do_stuff() {
        Ok(()) => {}
        _ => unreachable!(),
    }
}
