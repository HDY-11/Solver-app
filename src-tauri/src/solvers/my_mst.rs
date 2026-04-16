use std::collections::HashMap;
use std::mem::swap;
use std::vec;

use crate::City;

type HashMapType = HashMap<usize, Vec<usize>>;

struct UnionFind {
    parent: Vec<usize>,
}
impl UnionFind {
    fn new(size: usize) -> Self {
        UnionFind {
            parent: (0..size).collect(),
        }
    }

    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }

    fn union(&mut self, x: usize, y: usize) {
        let root_x = self.find(x);
        let root_y = self.find(y);
        if root_x != root_y {
            self.parent[root_y] = root_x;
        }
    }
}

struct HashMapWrapper(HashMapType);
/// MST+DFS序算法求解TSP
///
/// # 参数
/// - `cities`: 城市列表
/// - `adjacency_list`: 邻接表，每个元素为 Vec<(usize, f64)>
/// - `start_city`: 起始城市索引，如果为None则从城市0开始
///
/// # 返回
/// 返回访问顺序的Vec<usize>（城市索引序列）
pub fn my_mst_tsp(
    cities: &[City],
    adjacency_list: &[Vec<(usize, f64)>],
    start_city: Option<usize>,
) -> Vec<usize> {
    let n = cities.len();
    if n == 0 {
        return Vec::new();
    }

    let start = start_city.unwrap_or(0);
    let mut union_find = UnionFind::new(n);
    let mut path = Vec::with_capacity(n);

    // 生成MST

    let mut result = Vec::new();
    for i in 0..n {
        for &(neighbor, distance) in &adjacency_list[i] {
            result.push((i, neighbor, distance));
        }
    }
    // 按照距离排序边
    result.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());
    let mut edge_count = 0usize;
    let mut mst_edges = HashMapWrapper(HashMap::new());

    for (mut u, mut v, _) in result {
        // 始终规定小序号为父,构建有向树
        if v < u {
            swap(&mut u, &mut v);
        }
        // 成环跳过
        if union_find.find(u) == union_find.find(v) {
            continue;
        }
        // 大序号认父,合并集合
        mst_edges.0.entry(u).or_insert(Vec::new()).push(v);
        mst_edges.0.entry(v).or_insert(Vec::new()).push(u);

        union_find.union(u, v);
        edge_count += 1;
        if edge_count == n - 1 {
            break;
        }
    }

    // DFS遍历MST
    let mut current: usize;
    let mut stack = vec![start];
    let mut visited = vec![false; n];
    while !stack.is_empty() {
        current = stack.pop().unwrap();
        if !visited[current] {
            visited[current] = true;
            path.push(current);
            if let Some(neighbors) = mst_edges.0.get(&current) {
                stack.extend(neighbors.iter().rev());
            }
        }
    }

    path.push(start);
    path
}
