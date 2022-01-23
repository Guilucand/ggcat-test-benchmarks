use rayon::prelude::*;
use std::collections::VecDeque;
use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::io::{BufRead, Read, Write};
use std::iter::FromIterator;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::atomic::{AtomicU64, Ordering};
use structopt::StructOpt;

fn read_lines<'a, P>(
    filename: P,
    buffer: &'a mut Vec<u8>,
) -> io::Result<Box<dyn Iterator<Item = &'a mut str> + 'a>>
where
    P: AsRef<Path>,
{
    let mut file = File::open(filename.as_ref())?;

    if filename.as_ref().extension() == Some(OsStr::new("lz4")) {
        lz4::Decoder::new(file)
            .unwrap()
            .read_to_end(buffer)
            .unwrap();
    } else {
        file.read_to_end(buffer).unwrap();
    }

    let mut position = 0;

    let buffer_ptr = buffer as *mut Vec<u8>;

    Ok(Box::new(std::iter::from_fn(move || {
        let buffer = unsafe { &mut *(buffer_ptr) };

        if position >= buffer.len() {
            return None;
        }

        let last_position = position;
        let mut next_position = position;
        while next_position < buffer.len() && buffer[next_position] != b'\n' {
            next_position += 1;
        }
        position = next_position + 1;
        Some(std::str::from_utf8_mut(&mut buffer[last_position..next_position]).unwrap())
    })))
}

fn write_lines<'a, P>(filename: P, lines: impl Iterator<Item = &'a str>)
where
    P: AsRef<Path>,
{
    let file = File::create(filename).unwrap();
    let mut buffer = io::BufWriter::new(file);

    for line in lines {
        buffer.write_all(line.as_bytes()).unwrap();
        buffer.write_all(b"\n").unwrap();
    }
}

fn rcb(base: u8) -> u8 {
    match base {
        b'A' => b'T',
        b'C' => b'G',
        b'G' => b'C',
        b'T' => b'A',
        _ => panic!("Unknown base {}!", base as char),
    }
}

fn reverse_complement(s: &mut [u8]) {
    s.reverse();
    s.iter_mut().for_each(|x| *x = rcb(*x));
}

fn process_string(el: &mut [u8]) {
    let a = el.iter();
    let b = el.iter().rev();

    let should_swap = a
        .zip(b)
        .filter(|(a, b)| **a != rcb(**b))
        .next()
        .map(|(a, b)| *a > rcb(*b))
        .unwrap_or(false);

    if should_swap {
        reverse_complement(el);
    }
}

pub fn canonicalize(input: impl AsRef<Path>, output: impl AsRef<Path>, k: usize) {
    let total_kmers = AtomicU64::new(0);

    let mut buffer = Vec::new();

    let mut sequences: Vec<_> = read_lines(&input, &mut buffer)
        .unwrap()
        .filter(|l| !l.starts_with(">"))
        .collect();

    sequences.par_iter_mut().for_each(|el| {
        let str_bytes = el.as_bytes();

        total_kmers.fetch_add((str_bytes.len() - k + 1) as u64, Ordering::Relaxed);

        // Circular normalization
        if &str_bytes[..(k - 1)] == &str_bytes[(str_bytes.len() - (k - 1))..] {
            let mut canonical = el.to_string();
            process_string(unsafe { canonical.as_bytes_mut() });

            let mut deque = VecDeque::from_iter(str_bytes.iter().map(|x| *x));

            if deque.len() < k {
                println!("Sequence: {} has length less than k, aborting!", el);
                exit(1);
            }

            for _ in 0..str_bytes.len() {
                // Roll the sequence by 1 left
                let ins_el = deque[deque.len() - k];
                deque.push_front(ins_el);
                deque.pop_back();

                let mut candidate = std::str::from_utf8(deque.make_contiguous())
                    .unwrap()
                    .to_string();

                process_string(unsafe { candidate.as_bytes_mut() });
                canonical = canonical.min(candidate);
            }
            unsafe { el.as_bytes_mut().copy_from_slice(canonical.as_bytes()) };
        }
        process_string(unsafe { el.as_bytes_mut() });
    });

    sequences.par_sort();

    sequences.push(std::str::from_utf8_mut(&mut []).unwrap());
    write_lines(output, sequences.iter().map(|s| &**s));

    println!(
        "Written {} sequences with {} kmers!",
        sequences.len(),
        total_kmers.load(Ordering::Relaxed)
    );
}
