use crate::City;

/// 贪心算法（最近邻法）求解TSP
///
/// # 参数
/// - `cities`: 城市列表
/// - `adjacency_list`: 邻接表，每个元素为 Vec<(usize, f64)>
/// - `start_city`: 起始城市索引，如果为None则从城市0开始
///
/// # 返回
/// 返回访问顺序的Vec<usize>（城市索引序列）
pub fn greedy_tsp(
    cities: &[City],
    adjacency_list: &[Vec<(usize, f64)>],
    start_city: Option<usize>,
) -> Vec<usize> {
    let n = cities.len();
    if n == 0 {
        return Vec::new();
    }

    let start = start_city.unwrap_or(0);
    let mut visited = vec![false; n];
    let mut path = Vec::with_capacity(n);
    let mut current = start;

    path.push(current);
    visited[current] = true;

    for _ in 1..n {
        // 找到未访问的最近城市
        let mut next_city = None;
        let mut min_distance = f64::INFINITY;

        for &(neighbor, distance) in &adjacency_list[current] {
            if !visited[neighbor] && distance < min_distance {
                min_distance = distance;
                next_city = Some(neighbor);
            }
        }

        // 理论上应该总能找到未访问城市
        if let Some(next) = next_city {
            current = next;
            path.push(current);
            visited[current] = true;
        } else {
            break;
        }
    }

    path
}

/// 贪心算法（尝试所有起点，选择最优解）
///
/// 从每个城市作为起点运行贪心算法，返回总距离最短的路径
pub fn greedy_tsp_best_start(
    cities: &[City],
    adjacency_list: &[Vec<(usize, f64)>],
) -> (Vec<usize>, f64) {
    let n = cities.len();
    if n == 0 {
        return (Vec::new(), 0.0);
    }

    let mut best_path = Vec::new();
    let mut best_distance = f64::INFINITY;

    for start in 0..n {
        let path = greedy_tsp(cities, adjacency_list, Some(start));
        let distance = calculate_path_distance(&path, adjacency_list);

        if distance < best_distance {
            best_distance = distance;
            best_path = path;
        }
    }

    (best_path, best_distance)
}

/// 计算路径总距离（包括回到起点的距离）
pub fn calculate_path_distance(path: &[usize], adjacency_list: &[Vec<(usize, f64)>]) -> f64 {
    if path.is_empty() {
        return 0.0;
    }

    let mut total_distance = 0.0;

    // 计算相邻城市之间的距离
    for i in 0..path.len() - 1 {
        let from = path[i];
        let to = path[i + 1];

        // 在邻接表中查找距离
        if let Some(&(_, dist)) = adjacency_list[from].iter().find(|&&(city, _)| city == to) {
            total_distance += dist;
        }
    }

    // 添加回到起点的距离（形成回路）
    if path.len() > 1 {
        let last = *path.last().unwrap();
        let first = path[0];
        if let Some(&(_, dist)) = adjacency_list[last]
            .iter()
            .find(|&&(city, _)| city == first)
        {
            total_distance += dist;
        }
    }

    total_distance
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    fn create_test_cities(
        n: usize,
        width: f64,
        height: f64,
    ) -> (Vec<City>, Vec<Vec<(usize, f64)>>) {
        let mut rng = rand::thread_rng();
        let mut cities = Vec::new();

        for i in 0..n {
            cities.push(City {
                id: i,
                x: rng.gen_range(0.0..width),
                y: rng.gen_range(0.0..height),
            });
        }

        let mut adjacency_list = Vec::new();
        for i in 0..n {
            let mut edges = Vec::new();
            for j in 0..n {
                if i != j {
                    let dx = cities[i].x - cities[j].x;
                    let dy = cities[i].y - cities[j].y;
                    let distance = (dx * dx + dy * dy).sqrt();
                    edges.push((j, distance));
                }
            }
            adjacency_list.push(edges);
        }

        (cities, adjacency_list)
    }

    #[test]
    fn test_greedy_tsp() {
        let (cities, adj) = create_test_cities(5, 100.0, 100.0);
        let path = greedy_tsp(&cities, &adj, Some(0));

        // 检查路径长度
        assert_eq!(path.len(), 5);

        // 检查所有城市都被访问一次
        let mut visited = vec![false; 5];
        for &city in &path {
            visited[city] = true;
        }
        assert!(visited.iter().all(|&v| v));
    }

    #[test]
    fn test_calculate_path_distance() {
        let (_, adj) = create_test_cities(3, 100.0, 100.0);
        let path = vec![0, 1, 2];
        let distance = calculate_path_distance(&path, &adj);

        // 距离应该是正数
        assert!(distance > 0.0);
    }
}
