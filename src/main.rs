#![allow(unused_parens)]

use bitintr::Pdep;
use chrono::Local;
use rand::seq::SliceRandom;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io::BufRead;
use std::io;

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

fn f_live(live: bool, nh: usize) -> bool {
    match live {
        true => (nh == 3 || nh == 4),
        false => (nh == 3),
    }
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

        f_live(live, nh)
    }).collect()
}

fn strip_search(ww: isize, hh: isize, get_pat0: impl Fn(isize, isize) -> bool, is_rotor: impl Fn(isize, isize) -> bool, allowed_snh: impl Fn(isize, isize, bool, usize) -> bool) -> HashSet<(isize, isize)> {
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

    let mut rr: HashMap<(usize, usize), _> = HashMap::new();
    rr.insert((0, 0), (0, vec![]));

    for x in 2..ww {
        // debug_log(format!("x = {}, rr.len() = {}", x, rr.len()));
        let mut rr2 = HashMap::new();
        for ((c0, c1), (ct, cols)) in rr.into_iter() {
            'c2: for c2_raw in 0..(1 << c_raw_lens[x as usize]) {
                let c2 = c_outers[x as usize] | (c2_raw.pdep(c_inner_masks[x as usize]) as usize);

                for y in 1..(hh - 1) {
                    let live = (c1 & (1 << y)) != 0;
                    let mask = 7 << (y - 1);
                    let snh = (c0 & mask).count_ones() + (c1 & mask).count_ones() + (c2 & mask).count_ones();
                    let snh = snh as usize;
                    if !allowed_snh(x - 1, y, live, snh) {
                        continue 'c2;
                    }
                }

                let ct_next = ct + (c0.count_ones() as usize);
                if let Some(&(ct_already, _)) = rr2.get(&(c1, c2)) {
                    if ct_already <= ct_next {
                        continue 'c2;
                    }
                }

                let mut cols_next = cols.clone();
                cols_next.push(c0);
                rr2.insert((c1, c2), (ct_next, cols_next));
            }
        }
        rr = rr2;
    }

    match rr.get(&(0, 0)) {
        Some(&(_, ref cols)) => {
            cols.iter().enumerate().flat_map(|(x, col)| {
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

fn main() {
    let mut args = std::env::args().skip(1);
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
                min_x - search_max - 1,
                max_x + search_max + 1,
                min_y - search_max - 1,
                max_y + search_max + 1,
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
                [false, true].iter().map(|&live| {
                    // these triples are: (current liveness, rotor neighborhoos cell count, future liveness)
                    let triples = pats.iter().enumerate().map(|(i, pat)| {
                        let fpat = &pats[(i + 1) % pats.len()];
                        (
                            pat.contains(&(x, y)),
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
                            if live {
                                return false;
                            }
                            else {
                                return triples.iter().all(|&(live, rnh, flive)| f_live(live, snh + rnh) == flive);
                            }
                        }
                        else {
                            return triples.iter().all(|&(_, rnh, _)| f_live(live, snh + rnh) == live);
                        }
                    }).collect::<Vec<_>>()
                }).collect::<Vec<_>>()
            }).collect::<Vec<_>>()
        }).collect::<Vec<_>>()
    });
    // dbg!(&allowed_snh);

    let mut pat1 = pat0;
    loop {
        let mut progress = false;

        // horizontal stripes
        {
            let mut search_starts = (0..=(hh - (search_max + 4))).into_iter().collect::<Vec<_>>();
            search_starts.shuffle(&mut rand::thread_rng());

            for search_start in search_starts {
                let search_end = search_start + search_max + 4;

                let st1 = debug_time(format!("horizontal stripe [{}, {})", search_start, search_end), || {
                    strip_search(ww, search_max + 4, |x, y| {
                        pat1.contains(&(x, y + search_start))
                    }, |x, y| {
                        is_rotor[x as usize][(y + search_start) as usize]
                    }, |x, y, live, snh| {
                        allowed_snh[x as usize][(y + search_start) as usize][if live { 1 } else { 0 }][snh]
                    }).into_iter().map(|(x, y)| {
                        (x, y + search_start)
                    }).collect::<HashSet<_>>()
                });

                let pat2 = (0..ww).flat_map(|x| {
                    let is_rotor = &is_rotor;
                    let pat1 = &pat1;
                    let st1 = &st1;
                    (0..hh).filter(move |&y| {
                        if y < search_start || y >= search_end || is_rotor[x as usize][y as usize] {
                            return pat1.contains(&(x, y));
                        }
                        st1.contains(&(x, y))
                    }).map(move |y| {
                        (x, y)
                    })
                }).collect::<HashSet<_>>();

                if pat2.len() < pat1.len() {
                    eprintln!("Replace: {} -> {}", pat1.len(), pat2.len());
                    for y in 0..(search_start + 2) {
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
                    eprintln!("   {}", (0..ww).map(|_| '-').collect::<String>());
                    for y in (search_start + 2)..(search_end - 2) {
                        let s = (0..ww).map(|x| {
                            match (is_rotor[x as usize][y as usize], pat1.contains(&(x, y)), pat2.contains(&(x, y))) {
                                (true, false, false) => 'r',
                                (true, true, true) => 'R',
                                (false, true, true) => '*',
                                (false, true, false) => 'x',
                                (false, false, true) => 'o',
                                (false, false, false) => '.',
                                _ => panic!(),
                            }
                        }).collect::<String>();
                        eprintln!("   {}", s);
                    }
                    eprintln!("   {}", (0..ww).map(|_| '-').collect::<String>());
                    for y in (search_end - 2)..hh {
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

                    pat1 = pat2;
                    progress = true;
                }
            }
        }

        // vertical stripes
        {
            let mut search_starts = (0..=(ww - (search_max + 4))).into_iter().collect::<Vec<_>>();
            search_starts.shuffle(&mut rand::thread_rng());

            for search_start in search_starts {
                let search_end = search_start + search_max + 4;

                let st1 = debug_time(format!("vertical stripe [{}, {})", search_start, search_end), || {
                    strip_search(hh, search_max + 4, |y, x| {
                        pat1.contains(&(x + search_start, y))
                    }, |y, x| {
                        is_rotor[(x + search_start) as usize][y as usize]
                    }, |y, x, live, snh| {
                        allowed_snh[(x + search_start) as usize][y as usize][if live { 1 } else { 0 }][snh]
                    }).into_iter().map(|(y, x)| {
                        (x + search_start, y)
                    }).collect::<HashSet<_>>()
                });

                let pat2 = (0..ww).flat_map(|x| {
                    let is_rotor = &is_rotor;
                    let pat1 = &pat1;
                    let st1 = &st1;
                    (0..hh).filter(move |&y| {
                        if x < search_start || x >= search_end || is_rotor[x as usize][y as usize] {
                            return pat1.contains(&(x, y));
                        }
                        st1.contains(&(x, y))
                    }).map(move |y| {
                        (x, y)
                    })
                }).collect::<HashSet<_>>();

                if pat2.len() < pat1.len() {
                    eprintln!("Replace: {} -> {}", pat1.len(), pat2.len());
                    for y in 0..hh {
                        let s1 = (0..(search_start + 2)).map(|x| {
                            match (is_rotor[x as usize][y as usize], pat1.contains(&(x, y))) {
                                (true, false) => 'r',
                                (true, true) => 'R',
                                (false, false) => '.',
                                (false, true) => '*',
                            }
                        }).collect::<String>();
                        let s2 = ((search_start + 2)..(search_end - 2)).map(|x| {
                            match (is_rotor[x as usize][y as usize], pat1.contains(&(x, y)), pat2.contains(&(x, y))) {
                                (true, false, false) => 'r',
                                (true, true, true) => 'R',
                                (false, true, true) => '*',
                                (false, true, false) => 'x',
                                (false, false, true) => 'o',
                                (false, false, false) => '.',
                                _ => panic!(),
                            }
                        }).collect::<String>();
                        let s3 = ((search_end - 2)..ww).map(|x| {
                            match (is_rotor[x as usize][y as usize], pat1.contains(&(x, y))) {
                                (true, false) => 'r',
                                (true, true) => 'R',
                                (false, false) => '.',
                                (false, true) => '*',
                            }
                        }).collect::<String>();
                        eprintln!("   {}|{}|{}", s1, s2, s3);
                    }

                    pat1 = pat2;
                    progress = true;
                }
            }
        }

        if !progress {
            break;
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
