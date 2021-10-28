#![allow(unused_parens)]

use std::collections::HashMap;
use std::collections::HashSet;
use std::io::BufRead;
use std::io;

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

fn strip_search(ww: isize, hh: isize, fixed: impl Fn(isize, isize) -> bool, allowed_snh: impl Fn(isize, isize, bool, usize) -> bool) -> HashSet<(isize, isize)> {
    eprintln!("Strip searching:");
    for y in 0..hh {
        let s = (0..ww).map(|x| {
            if fixed(x, y) { '*' } else { '.' }
        }).collect::<String>();
        eprintln!("   {}", s);
    }

    for y in 0..hh {
        for x in 0..2 {
            assert!(!fixed(x, y));
        }
    }

    let c_outers = (0..ww).map(|x| {
        let mut c_outer = 0usize;
        for &y in &[0, 1, hh - 2, hh - 1] {
            if fixed(x, y) {
                c_outer |= (1 << y);
            }
        }
        c_outer
    }).collect::<Vec<_>>();

    let mut rr: HashMap<(usize, usize), _> = HashMap::new();
    rr.insert((0, 0), (0, vec![]));

    for x in 2..ww {
        eprintln!("x = {}, rr.len() = {}", x, rr.len());
        let mut rr2 = HashMap::new();
        for ((c0, c1), (ct, cols)) in rr.into_iter() {
            'c2: for c2_inner in 0..(1 << ((2 * (hh - 4)) as usize)) {
                let c2 = c_outers[x as usize] | (c2_inner << 2);

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

    let pats = {
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
    };

    // dbg!(pats);

    let (pat0, pats, ww, hh) = {
        let (bb_min_x, bb_max_x, bb_min_y, bb_max_y) = {
            let all_cells: HashSet<_> = pats.iter().flat_map(|pat| {
                pat.iter().map(|&p| p)
            }).collect();
            let min_x = all_cells.iter().map(|&(x, _)| x).min().unwrap();
            let max_x = all_cells.iter().map(|&(x, _)| x).max().unwrap();
            let min_y = all_cells.iter().map(|&(_, y)| y).min().unwrap();
            let max_y = all_cells.iter().map(|&(_, y)| y).max().unwrap();
            let w = max_x - min_x + 1;
            let h = max_y - min_y + 1;
            (
                min_x - w - 2,
                max_x + w + 2,
                min_y - h - 2,
                max_y + h + 2,
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
    };

    // dbg!(pats, ww, hh);

    let is_rotor = (0..ww).map(|x| {
        (0..hh).map(|y| {
            let min = pats.iter().map(|pat| pat.contains(&(x, y))).min().unwrap();
            let max = pats.iter().map(|pat| pat.contains(&(x, y))).max().unwrap();
            min != max
        }).collect::<Vec<_>>()
    }).collect::<Vec<_>>();

    // dbg!(is_rotor);

    let allowed_snh = (0..ww).map(|x| {
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
    }).collect::<Vec<_>>();

    // dbg!(allowed_snh);
}
