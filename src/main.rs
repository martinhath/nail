extern crate image;

use std::env;

use image::{Pixel, ImageBuffer, Rgb, ConvertBuffer};

// type Image = ImageBuffer<Rgb<f32>, Vec<f32>>;
type Image = ImageBuffer<Rgb<u8>, Vec<u8>>;

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
    let image: Image = image::open(&filename).unwrap().to_rgb();
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
        *Rgb::from_slice(&[b[0] as u8, b[1] as u8, b[2] as u8])
    };
    let mut buffer: Image =
        image::ImageBuffer::new(image.width(), image.height());
    for (_x, _y, p) in buffer.enumerate_pixels_mut() {
        *p = avg_color;
    }

    println!("rmse: {}", rmse(&image, &buffer));

    buffer.save(&"output.png");
}
