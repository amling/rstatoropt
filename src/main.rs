use std::collections::HashMap;
use std::collections::HashSet;
use std::io::BufRead;
use std::io;

fn step_pat(pat: &HashSet<(isize, isize)>) -> HashSet<(isize, isize)> {
    let check: HashSet<_> = pat.iter().flat_map(|&(x, y)| {
        (-1..2).flat_map(move |dx| {
            (-1..2).map(move |dy| {
                (x + dx, y + dy)
            })
        })
    }).collect();

    check.into_iter().filter(|&(x2, y2)| {
        let live = pat.contains(&(x2, y2));
        let nh: usize = (-1..2).map(|dx| {
            (-1..2).filter(|dy| {
                pat.contains(&(x2 + dx, y2 + dy))
            }).count()
        }).sum();

        match live {
            true => (nh == 3 || nh == 4),
            false => (nh == 3),
        }
    }).collect()
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

    let (pats, ww, hh) = {
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
                min_x - w,
                max_x + w,
                min_y - h,
                max_y + h,
            )
        };

        (
            pats.into_iter().map(|pat| {
                pat.into_iter().map(|(x, y)| (x - bb_min_x, y - bb_min_y)).collect::<HashSet<_>>()
            }).collect::<Vec<_>>(),
            (bb_max_x - bb_min_x + 1),
            (bb_max_y - bb_min_y + 1),
        )
    };

    // dbg!(pats, ww, hh);
}
