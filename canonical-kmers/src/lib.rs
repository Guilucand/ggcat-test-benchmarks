use rayon::prelude::*;
use regex::Regex;
use std::collections::{HashMap, VecDeque};
use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::io::{Read, Write};
use std::iter::FromIterator;
use std::path::Path;
use std::process::exit;
use std::sync::atomic::{AtomicU64, Ordering};

fn read_fasta_lines<'a, P>(
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

        let is_ident = buffer[position] == b'>';

        let mut copy_index = next_position;

        // Iterate one or more lines
        while next_position < buffer.len() {
            // Read a whole line (including the newline)
            while next_position < buffer.len() && buffer[next_position] != b'\n' {
                buffer[copy_index] = buffer[next_position];
                copy_index += 1;
                next_position += 1;
            }
            next_position += 1;

            if is_ident {
                break;
            } else if next_position < buffer.len() && buffer[next_position] == b'>' {
                break;
            }
        }

        position = next_position;
        Some(std::str::from_utf8_mut(&mut buffer[last_position..copy_index]).unwrap())
    })))
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

fn process_string(el: &mut [u8]) -> bool {
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
    should_swap
}

pub fn canonicalize(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    k: usize,
    normalize_links: bool,
) {
    let total_kmers = AtomicU64::new(0);

    let mut buffer = Vec::new();

    let mut lines: Vec<_> = read_fasta_lines(&input, &mut buffer).unwrap().collect();
    let mut sequences: Vec<_> = lines
        .chunks_exact_mut(2)
        .map(|s| {
            (
                std::mem::take(&mut s[0]),
                std::mem::take(&mut s[1]),
                vec![],
                0,
                false,
                false,
            )
        })
        .collect();

    let ident_parser = Regex::new(r"^>(\d+)").unwrap();
    let link_parser = Regex::new(r"L:([+-]):(\d+):([+-])").unwrap();

    sequences.par_iter_mut().for_each(
        |(ident, sequence, links, original_index, flipped, circular)| {
            let str_bytes = sequence.as_bytes();

            total_kmers.fetch_add((str_bytes.len() - k + 1) as u64, Ordering::Relaxed);

            if str_bytes.len() < k {
                println!("Sequence: {} has length less than k, aborting!", sequence);
                exit(1);
            }

            if normalize_links {
                let groups = ident_parser.captures(ident).unwrap();
                let index = groups.get(1).unwrap().as_str().parse::<usize>().unwrap();
                *original_index = index;

                link_parser.captures_iter(ident).for_each(|link| {
                    let flip_current = link.get(1).unwrap().as_str().as_bytes()[0] == b'-';
                    let next_index = link.get(2).unwrap().as_str().parse::<usize>().unwrap();
                    let flip_next = link.get(3).unwrap().as_str().as_bytes()[0] == b'-';
                    links.push((flip_current, next_index, flip_next));
                });
            }

            // Detect rc circularity
            if str_bytes[..(k - 1)]
                .iter()
                .zip(str_bytes[..(k - 1)].iter().rev().map(|x| rcb(*x)))
                .all(|(a, b)| *a == b)
            {
                *circular = true;
            }

            // Detect rc circularity
            if str_bytes[(str_bytes.len() - (k - 1))..]
                .iter()
                .zip(
                    str_bytes[(str_bytes.len() - (k - 1))..]
                        .iter()
                        .rev()
                        .map(|x| rcb(*x)),
                )
                .all(|(a, b)| *a == b)
            {
                *circular = true;
            }

            // Circular normalization
            if &str_bytes[..(k - 1)] == &str_bytes[(str_bytes.len() - (k - 1))..] {
                *circular = true;
                let mut canonical = sequence.to_string();
                let mut reverse_complemented = process_string(unsafe { canonical.as_bytes_mut() });

                let mut deque = VecDeque::from_iter(str_bytes.iter().map(|x| *x));

                for _ in 0..str_bytes.len() {
                    // Roll the sequence by 1 left
                    let ins_el = deque[deque.len() - k];
                    deque.push_front(ins_el);
                    deque.pop_back();

                    let mut candidate = std::str::from_utf8(deque.make_contiguous())
                        .unwrap()
                        .to_string();

                    let new_reverse_complemented =
                        process_string(unsafe { candidate.as_bytes_mut() });

                    if candidate < canonical {
                        reverse_complemented = new_reverse_complemented;
                        canonical = candidate;
                    }
                }
                unsafe {
                    sequence
                        .as_bytes_mut()
                        .copy_from_slice(canonical.as_bytes())
                };
                *flipped = reverse_complemented;
            } else {
                *flipped = process_string(unsafe { sequence.as_bytes_mut() });
            }
        },
    );

    sequences.par_sort_by(|a, b| a.1.cmp(&b.1));

    let mut idents_buffer = vec![];

    let mut indexes_mappings = HashMap::new();
    let mut flipped_status = vec![];
    if normalize_links {
        for (new_index, sequence) in sequences.iter_mut().enumerate() {
            indexes_mappings.insert(sequence.3, new_index);
            sequence.3 = new_index;
            flipped_status.push(sequence.4);
        }
    }

    let output_file = File::create(output).unwrap();
    let mut output_file = io::BufWriter::new(output_file);

    for (new_index, sequence) in sequences.iter_mut().enumerate() {
        if normalize_links {
            sequence
                .2
                .iter_mut()
                .for_each(|(flip_current, next_index, flip_next)| {
                    *flip_current = *flip_current ^ sequence.4;
                    *next_index = *indexes_mappings.get(next_index).unwrap();
                    *flip_next = *flip_next ^ flipped_status[*next_index];
                });

            idents_buffer.clear();
            write!(idents_buffer, ">{}", new_index).unwrap();
            sequence.2.sort();
            for (flip_current, next_index, flip_next) in sequence.2.iter() {
                write!(
                    idents_buffer,
                    " L:{}:{}:{}",
                    if *flip_current { '-' } else { '+' },
                    next_index,
                    if *flip_next { '-' } else { '+' }
                )
                .unwrap();
            }

            if sequence.5 {
                write!(idents_buffer, " CIRCULAR").unwrap();
            }

            writeln!(idents_buffer).unwrap();
            output_file.write_all(&idents_buffer).unwrap();
        }
        output_file.write_all(sequence.1.as_bytes()).unwrap();
        output_file.write_all(&[b'\n']).unwrap();
    }

    drop(output_file);

    println!(
        "Written {} sequences with {} kmers!",
        sequences.len(),
        total_kmers.load(Ordering::Relaxed)
    );
}
