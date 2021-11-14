#![allow(unused_parens)]

use bitintr::Pdep;
use bitintr::Pext;
use chrono::Local;
use rand::seq::SliceRandom;
use rayon::prelude::*;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::hash::Hash;
use std::io::BufRead;
use std::io;
use std::sync::Arc;

pub fn debug_log(msg: impl AsRef<str>) {
    let msg = msg.as_ref();
    eprintln!("{} - {}", Local::now().format("%Y%m%d %H:%M:%S"), msg);
}

pub fn debug_time<T>(label: impl AsRef<str>, cb: impl FnOnce() -> T) -> T {
    let label = label.as_ref();
    let t0 = std::time::Instant::now();
    debug_log(format!("Starting {}...", label));
    let ret = cb();
    debug_log(format!("Finished {}: {:?}", label, t0.elapsed()));
    return ret;
}

fn f_live(live: usize, nh: usize) -> bool {
    let magic_ct = 2 * nh + 1 - live;
    6 <= magic_ct && magic_ct <= 8
}

fn step_pat(pat: &HashSet<(isize, isize)>) -> HashSet<(isize, isize)> {
    let check: HashSet<_> = pat.iter().flat_map(|&(x, y)| {
        (-1..=1).flat_map(move |dx| {
            (-1..=1).map(move |dy| {
                (x + dx, y + dy)
            })
        })
    }).collect();

    check.into_iter().filter(|&(x2, y2)| {
        let live = pat.contains(&(x2, y2));
        let nh: usize = (-1..=1).map(|dx| {
            (-1..=1).filter(|dy| {
                pat.contains(&(x2 + dx, y2 + dy))
            }).count()
        }).sum();

        f_live(if live { 1 } else { 0 }, nh)
    }).collect()
}

#[derive(Clone)]
struct ColList(Arc<Option<(ColList, usize)>>);

impl ColList {
    pub fn new() -> Self {
        ColList(Arc::new(None))
    }

    pub fn append(&self, col: usize) -> Self {
        ColList(Arc::new(Some((self.clone(), col))))
    }

    pub fn materialize(&self) -> Vec<usize> {
        match *self.0 {
            None => vec![],
            Some((ref prev, col)) => {
                let mut r = prev.materialize();
                r.push(col);
                r
            },
        }
    }
}

fn strip_search<'a>(ww: isize, hh: isize, get_pat0: impl Fn(isize, isize) -> bool, is_rotor: impl Fn(isize, isize) -> bool, allowed_snh: impl Fn(isize, isize) -> &'a Vec<Vec<bool>>) -> HashSet<(isize, isize)> {
    // eprintln!("Strip searching:");
    // for y in 0..hh {
    //     let s = (0..ww).map(|x| {
    //         if get_pat0(x, y) { '*' } else { '.' }
    //     }).collect::<String>();
    //     eprintln!("   {}", s);
    // }

    for y in 0..hh {
        for x in 0..2 {
            assert!(!get_pat0(x, y));
        }
    }

    let c_outers = (0..ww).map(|x| {
        let mut c_outer = 0usize;
        for &y in &[0, 1, hh - 2, hh - 1] {
            if !is_rotor(x, y) && get_pat0(x, y) {
                c_outer |= (1 << y);
            }
        }
        c_outer
    }).collect::<Vec<_>>();

    let c_inner_masks = (0..ww).map(|x| {
        (2..(hh - 2)).filter(|&y| {
            !is_rotor(x, y)
        }).map(|y| {
            1 << y
        }).sum::<u64>()
    }).collect::<Vec<_>>();

    let c_raw_lens = c_inner_masks.iter().map(|&mask| {
        mask.count_ones()
    }).collect::<Vec<_>>();

    let mut rr = vec![None; 1 << (c_raw_lens[0] + c_raw_lens[1])];
    rr[0] = Some((0, ColList::new()));

    for x in 2..ww {
        // debug_log(format!("x = {}, rr.len() = {}", x, rr.len()));
        let c0_raw_len = c_raw_lens[(x - 2) as usize];
        let c1_raw_len = c_raw_lens[(x - 1) as usize];
        let c2_raw_len = c_raw_lens[x as usize];
        let c0_outer = c_outers[(x - 2) as usize];
        let c1_outer = c_outers[(x - 1) as usize];
        let c2_outer = c_outers[x as usize];
        let c0_inner_mask = c_inner_masks[(x - 2) as usize];
        let c1_inner_mask = c_inner_masks[(x - 1) as usize];
        let c2_inner_mask = c_inner_masks[x as usize];

        let checks = (1..(hh - 1)).map(|y| {
            let allowed = (0..=1).map(|live| {
                (0..=9).filter(|&snh| {
                    allowed_snh(x - 1, y)[live][snh]
                }).map(|snh| 1 << snh).sum::<usize>()
            }).collect::<Vec<_>>();
            (y, allowed)
        }).collect::<Vec<_>>();

        let mut rr2 = vec![None; 1 << (c1_raw_len + c2_raw_len)];
        let rr2_chunks = rr2.chunks_mut(1 << c2_raw_len).collect::<Vec<_>>();
        rr2_chunks.into_par_iter().enumerate().for_each(|(c1_raw, rr2_slice)| {
            let c1_raw = c1_raw as u64;
            let c1 = c1_outer | (c1_raw.pdep(c1_inner_mask) as usize);

            // adjust checks for c1, namely (1) remove "live" dimension and (b) update allowed snhs
            // for contents of c1 column
            let checks = checks.iter().map(|&(y, ref allowed)| {
                let live = (c1 >> y) & 1;
                let mask = 7 << (y - 1);
                let c1_snh = (c1 & mask).count_ones();
                let allowed = (0..=6).filter(|snh| {
                    allowed[live] & (1 << (c1_snh + snh)) != 0
                }).map(|snh| 1 << snh).sum::<usize>();
                (y, allowed)
            }).collect::<Vec<_>>();

            let mut best = vec![None; 1 << c2_raw_len];
            for c0_raw in 0u64..(1 << c0_raw_len) {
                let (ct, cols) = match &rr[((c0_raw << c1_raw_len) | c1_raw) as usize] {
                    Some(r) => r,
                    None => {
                        continue;

                    }
                };
                let c0 = c0_outer | (c0_raw.pdep(c0_inner_mask) as usize);

                // last stop before the tight inner loop, adjust checks as hard as we can, namely
                // (1) adjust for contents of c0 columm, (2) adjust for contexts of c2_outer, (3)
                // remap bits of would-be mask to act on c2_raw instead of c2.
                let checks = checks.iter().map(|&(y, allowed)| {
                    let mask = 7 << (y - 1);
                    let c0_snh = (c0 & mask).count_ones();
                    let c2_snh_fixed = (c2_outer & mask).count_ones();
                    let c2_snh_raw_mask = (mask as u64).pext(c2_inner_mask);
                    let allowed = (0..=3).filter(|&snh| {
                        allowed & (1 << (c0_snh + c2_snh_fixed + snh)) != 0
                    }).map(|snh| 1 << snh).sum::<usize>();
                    (c2_snh_raw_mask, allowed)
                }).collect::<Vec<_>>();

                'c2: for c2_raw in 0..(1 << c2_raw_len) {
                    for &(c2_snh_raw_mask, allowed) in checks.iter() {
                        // We've precomputed these checks as absolutely hard as we can above, now
                        // it's just mask out [number of] relevant bits of c2_raw and see if it's a
                        // permitted number.
                        let c2_snh_raw = (c2_raw & c2_snh_raw_mask).count_ones();
                        if allowed & (1 << c2_snh_raw) == 0 {
                            continue 'c2;
                        }
                    }

                    let ct_next = ct + (c0.count_ones() as usize);
                    let p = &mut best[c2_raw as usize];
                    if let &mut Some((ct_already, _, _)) = p {
                        if ct_already <= ct_next {
                            continue 'c2;
                        }
                    }

                    *p = Some((ct_next, cols, c0));
                }
            }

            for (r, p2) in best.into_iter().zip(rr2_slice.iter_mut()) {
                *p2 = r.map(|(ct_next, cols, c0)| {
                    (ct_next, cols.append(c0))
                });
            }
        });

        rr = rr2;
    }

    match &rr[0] {
        &Some((_, ref cols)) => {
            cols.materialize().into_iter().enumerate().flat_map(|(x, col)| {
                (0..hh).filter(move |&y| {
                    (col & (1 << y)) != 0
                }).map(move |y| {
                    (x as isize, y)
                })
            }).collect()
        },
        None => panic!(),
    }
}

struct Queue<T> {
    queue: VecDeque<T>,
    set: HashSet<T>,
}

impl<T> Queue<T> {
    fn new() -> Self {
        Queue {
            queue: VecDeque::new(),
            set: HashSet::new(),
        }
    }

    fn add(&mut self, t: T) where T: Eq + Hash + Copy {
        if self.set.insert(t) {
            self.queue.push_back(t);
        }
    }

    fn pop(&mut self) -> Option<T> where T: Eq + Hash {
        self.queue.pop_front().map(|t| {
            assert!(self.set.remove(&t));
            t
        })
    }
}

#[derive(Clone)]
#[derive(Copy)]
#[derive(Eq)]
#[derive(Hash)]
#[derive(PartialEq)]
enum Search {
    HorizontalSearch(isize, isize),
    VerticalSearch(isize, isize),
}

impl Search {
    fn sees(&self, x: isize, y: isize) -> bool {
        match self {
            &Search::HorizontalSearch(start, end) => (start <= y && y <= end),
            &Search::VerticalSearch(start, end) => (start <= x && x <= end),
        }
    }

    fn search(&self, ww: isize, hh: isize, pat1: &HashSet<(isize, isize)>, is_rotor: &Vec<Vec<bool>>, allowed_snh: &Vec<Vec<Vec<Vec<bool>>>>) -> HashSet<(isize, isize)> {
        match self {
            &Search::HorizontalSearch(start, end) => {
                let st1 = debug_time(format!("horizontal stripe [{}, {})", start, end), || {
                    strip_search(ww, end - start, |x, y| {
                        pat1.contains(&(x, y + start))
                    }, |x, y| {
                        is_rotor[x as usize][(y + start) as usize]
                    }, |x, y| {
                        &allowed_snh[x as usize][(y + start) as usize]
                    }).into_iter().map(|(x, y)| {
                        (x, y + start)
                    }).collect::<HashSet<_>>()
                });

                let pat2 = (0..ww).flat_map(|x| {
                    let is_rotor = &is_rotor;
                    let pat1 = &pat1;
                    let st1 = &st1;
                    (0..hh).filter(move |&y| {
                        if y < start || y >= end || is_rotor[x as usize][y as usize] {
                            return pat1.contains(&(x, y));
                        }
                        st1.contains(&(x, y))
                    }).map(move |y| {
                        (x, y)
                    })
                }).collect::<HashSet<_>>();

                pat2
            },
            &Search::VerticalSearch(start, end) => {
                let st1 = debug_time(format!("vertical stripe [{}, {})", start, end), || {
                    strip_search(hh, end - start, |y, x| {
                        pat1.contains(&(x + start, y))
                    }, |y, x| {
                        is_rotor[(x + start) as usize][y as usize]
                    }, |y, x| {
                        &allowed_snh[(x + start) as usize][y as usize]
                    }).into_iter().map(|(y, x)| {
                        (x + start, y)
                    }).collect::<HashSet<_>>()
                });

                let pat2 = (0..ww).flat_map(|x| {
                    let is_rotor = &is_rotor;
                    let pat1 = &pat1;
                    let st1 = &st1;
                    (0..hh).filter(move |&y| {
                        if x < start || x >= end || is_rotor[x as usize][y as usize] {
                            return pat1.contains(&(x, y));
                        }
                        st1.contains(&(x, y))
                    }).map(move |y| {
                        (x, y)
                    })
                }).collect::<HashSet<_>>();

                pat2
            },
        }
    }

    fn display_delta(&self, ww: isize, hh: isize, display_char: impl Fn(isize, isize) -> char) {
        match self {
            &Search::HorizontalSearch(start, end) => {
                for y in 0..(start + 2) {
                    eprintln!("   {}", (0..ww).map(|x| display_char(x, y)).collect::<String>());
                }
                eprintln!("   {}", (0..ww).map(|_| '-').collect::<String>());
                for y in (start + 2)..(end - 2) {
                    eprintln!("   {}", (0..ww).map(|x| display_char(x, y)).collect::<String>());
                }
                eprintln!("   {}", (0..ww).map(|_| '-').collect::<String>());
                for y in (end - 2)..hh {
                    eprintln!("   {}", (0..ww).map(|x| display_char(x, y)).collect::<String>());
                }
            },
            &Search::VerticalSearch(start, end) => {
                for y in 0..hh {
                    let s1 = (0..(start + 2)).map(|x| display_char(x, y)).collect::<String>();
                    let s2 = ((start + 2)..(end - 2)).map(|x| display_char(x, y)).collect::<String>();
                    let s3 = ((end - 2)..ww).map(|x| display_char(x, y)).collect::<String>();
                    eprintln!("   {}|{}|{}", s1, s2, s3);
                }
            },
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let bb_pad: isize = args.next().unwrap().parse().unwrap();
    let search_max: isize = args.next().unwrap().parse().unwrap();

    let pat0 = debug_time("parse pat0", || {
        let mut pat0 = HashSet::new();
        for (y, line) in io::stdin().lock().lines().enumerate() {
            let line = line.unwrap();
            for (x, c) in line.chars().enumerate() {
                match c {
                    '.' => {},
                    '*' => {
                        pat0.insert((x as isize, y as isize));
                    },
                    _ => panic!(),
                }
            }
        }
        pat0
    });
    // dbg!(&pat0);

    let pats = debug_time("detect period", || {
        let mut pats = vec![];
        let mut pat_t = HashMap::new();
        let mut pat = pat0.clone();
        loop {
            let pat_sorted: Vec<_> = itertools::sorted(pat.clone()).collect();
            if let Some(&t0) = pat_t.get(&pat_sorted) {
                assert!(t0 == 0);
                break;
            }

            let t = pats.len();
            assert!(t < 1000);
            pats.push(pat.clone());
            pat_t.insert(pat_sorted, t);

            pat = step_pat(&pat);
        }
        pats
    });
    // dbg!(&pats);

    let (pat0, pats, ww, hh) = debug_time("bounding box", || {
        let (bb_min_x, bb_max_x, bb_min_y, bb_max_y) = {
            let all_cells: HashSet<_> = pats.iter().flat_map(|pat| {
                pat.iter().map(|&p| p)
            }).collect();
            let min_x = all_cells.iter().map(|&(x, _)| x).min().unwrap();
            let max_x = all_cells.iter().map(|&(x, _)| x).max().unwrap();
            let min_y = all_cells.iter().map(|&(_, y)| y).min().unwrap();
            let max_y = all_cells.iter().map(|&(_, y)| y).max().unwrap();
            (
                min_x - bb_pad - 2,
                max_x + bb_pad + 2,
                min_y - bb_pad - 2,
                max_y + bb_pad + 2,
            )
        };

        let shift_pat = |pat: HashSet<(isize, isize)>| {
            pat.into_iter().map(|(x, y)| (x - bb_min_x, y - bb_min_y)).collect::<HashSet<_>>()
        };

        (
            shift_pat(pat0),
            pats.into_iter().map(shift_pat).collect::<Vec<_>>(),
            (bb_max_x - bb_min_x + 1),
            (bb_max_y - bb_min_y + 1),
        )
    });
    // dbg!(ww, hh);

    let is_rotor = debug_time("is_rotor", || {
        (0..ww).map(|x| {
            (0..hh).map(|y| {
                let min = pats.iter().map(|pat| pat.contains(&(x, y))).min().unwrap();
                let max = pats.iter().map(|pat| pat.contains(&(x, y))).max().unwrap();
                min != max
            }).collect::<Vec<_>>()
        }).collect::<Vec<_>>()
    });
    // dbg!(&is_rotor);

    let allowed_snh = debug_time("allowed_snh", || {
        (0..ww).map(|x| {
            (0..hh).map(|y| {
                (0..=1).map(|live| {
                    // these triples are: (current liveness, rotor neighborhoos cell count, future liveness)
                    let triples = pats.iter().enumerate().map(|(i, pat)| {
                        let fpat = &pats[(i + 1) % pats.len()];
                        (
                            if pat.contains(&(x, y)) { 1 } else { 0 },
                            (-1..=1).map(|dx| {
                                let x2 = x + dx;
                                if x2 < 0 || x2 >= ww {
                                    return 0;
                                }
                                (-1..=1).filter(|dy| {
                                    let y2 = y + dy;
                                    if y2 < 0 || y2 >= hh {
                                        return false;
                                    }
                                    if !is_rotor[(x2 as usize)][(y2 as usize)] {
                                        return false;
                                    }
                                    pat.contains(&(x2, y2))
                                }).count()
                            }).sum::<usize>(),
                            fpat.contains(&(x, y)),
                        )
                    }).collect::<HashSet<_>>();
                    (0..=9).map(|snh| {
                        if is_rotor[x as usize][y as usize] {
                            if live != 0 {
                                return false;
                            }
                            else {
                                return triples.iter().all(|&(live, rnh, flive)| f_live(live, snh + rnh) == flive);
                            }
                        }
                        else {
                            return triples.iter().all(|&(_, rnh, _)| f_live(live, snh + rnh) == (live != 0));
                        }
                    }).collect::<Vec<_>>()
                }).collect::<Vec<_>>()
            }).collect::<Vec<_>>()
        }).collect::<Vec<_>>()
    });
    // dbg!(&allowed_snh);

    let mut all_searches = vec![];
    if search_max + 4 <= hh {
        for search_start in 0..=(hh - (search_max + 4)) {
            all_searches.push(Search::HorizontalSearch(search_start, search_start + search_max + 4));
        }
    }
    else {
        all_searches.push(Search::HorizontalSearch(0, hh));
    }
    if search_max + 4 <= ww {
        for search_start in 0..=(ww - (search_max + 4)) {
            all_searches.push(Search::VerticalSearch(search_start, search_start + search_max + 4));
        }
    }
    else {
        all_searches.push(Search::VerticalSearch(0, ww));
    }
    let all_searches = all_searches;


    let mut search_updates = HashMap::new();
    for x in 0..ww {
        for y in 0..hh {
            for &search in &all_searches {
                if search.sees(x, y) {
                    search_updates.entry((x, y)).or_insert_with(Vec::new).push(search);
                }
            }
        }
    }
    let search_updates = search_updates;


    let mut queue = Queue::new();
    {
        let mut initial_queue = all_searches.clone();
        initial_queue.shuffle(&mut rand::thread_rng());
        for search in initial_queue {
            queue.add(search);
        }
    }

    let mut pat1 = pat0;
    loop {
        let search = match queue.pop() {
            Some(search) => search,
            None => {
                break;
            }
        };

        let pat2 = search.search(ww, hh, &pat1, &is_rotor, &allowed_snh);

        if pat2.len() < pat1.len() {
            eprintln!("Replace: {} -> {}", pat1.len(), pat2.len());
            search.display_delta(ww, hh, |x, y| {
                match (is_rotor[x as usize][y as usize], pat1.contains(&(x, y)), pat2.contains(&(x, y))) {
                    (true, false, false) => 'r',
                    (true, true, true) => 'R',
                    (false, true, true) => '*',
                    (false, true, false) => 'x',
                    (false, false, true) => 'o',
                    (false, false, false) => '.',
                    _ => panic!(),
                }
            });

            // decide what searches we need to repeat
            let mut new_searches = HashSet::new();
            for (p, searches) in search_updates.iter() {
                if pat2.contains(&p) != pat1.contains(&p) {
                    for &search in searches {
                        new_searches.insert(search);
                    }
                }
            }
            let mut new_searches = new_searches.into_iter().collect::<Vec<_>>();
            new_searches.shuffle(&mut rand::thread_rng());
            for search in new_searches {
                queue.add(search);
            }

            pat1 = pat2;
        }
    }

    eprintln!("Final: {}", pat1.len());
    for y in 0..hh {
        let s = (0..ww).map(|x| {
            match (is_rotor[x as usize][y as usize], pat1.contains(&(x, y))) {
                (true, false) => 'r',
                (true, true) => 'R',
                (false, false) => '.',
                (false, true) => '*',
            }
        }).collect::<String>();
        eprintln!("   {}", s);
    }
}
