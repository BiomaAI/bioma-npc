use std::collections::VecDeque;

use crate::{PreWorldHookArgs, PreWorldHookFn};

pub fn islands_metric_hook() -> PreWorldHookFn {
    Box::new(|PreWorldHookArgs { world, .. }| {
        let width = world.map.width;
        let height = world.map.height;
        let mut visited = vec![false; width * height];
        let mut queue = VecDeque::new();
        let mut islands = 0;

        for y in 0..height {
            for x in 0..width {
                let index = width * y + x;
                if visited[index] || world.map.tiles[y][x].is_impassable() {
                    continue;
                }

                islands += 1;
                visited[index] = true;
                queue.push_back((x, y));

                while let Some((x, y)) = queue.pop_front() {
                    let neighbors = [
                        y.checked_sub(1).map(|y| (x, y)),
                        if y + 1 < height {
                            Some((x, y + 1))
                        } else {
                            None
                        },
                        x.checked_sub(1).map(|x| (x, y)),
                        if x + 1 < width {
                            Some((x + 1, y))
                        } else {
                            None
                        },
                    ];

                    for (nx, ny) in neighbors.into_iter().flatten() {
                        let neighbor_index = width * ny + nx;
                        if visited[neighbor_index] || world.map.tiles[ny][nx].is_impassable() {
                            continue;
                        }

                        visited[neighbor_index] = true;
                        queue.push_back((nx, ny));
                    }
                }
            }
        }

        println!("# of islands: {islands}");
    })
}
