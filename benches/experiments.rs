#![feature(test)]

extern crate test;
use test::Bencher;

fn basic(number: u32) -> u32 {
    match number {
        0..=9 => 1,
        10..=99 => 2,
        100..=999 => 3,
        1000..=9999 => 4,
        10000..=99999 => 5,
        100000..=999999 => 6,
        1000000..=9999999 => 7,
        10000000..=99999999 => 8,
        100000000..=999999999 => 9,
        1000000000..=4294967295 => 10
    }
}

#[bench]
fn bench_basic(b: &mut Bencher) {
    b.iter(|| {
        let a = test::black_box(123456789_u32);
        let b = test::black_box(basic(a));
    });
}

#[bench]
fn bench_iterative(b: &mut Bencher) {
    b.iter(|| {
        let mut count = test::black_box(0);
        let mut number = test::black_box(123456789_u32);
        while number != 0 {
            number /= 10;
            count += 1;
        }
    });
}

fn recursive(number: u32) -> u32 {
    if number == 0 {
        0
    }
    else {
        1 + recursive(number / 10)
    }
}

#[bench]
fn bench_recursive(b: &mut Bencher) {
    b.iter(|| {
        let a = test::black_box(123456789_u32);
        let b = test::black_box(recursive(a));
    });
}

#[bench]
fn bench_log(b: &mut Bencher) {
    b.iter(|| {
        let a = test::black_box(123456789_f32);
        let b = test::black_box(a.log10() as u32);
    });
}
