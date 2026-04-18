pub mod solvers {
    pub mod greedy;
    pub mod my_mst;
}

use rand::Rng;
use serde::{Deserialize, Serialize};
use tauri::command;

use crate::solvers::greedy::{calculate_path_distance, greedy_tsp, greedy_tsp_best_start};
use crate::solvers::my_mst::my_mst_tsp;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct City {
    pub id: usize,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TspData {
    pub cities: Vec<City>,
    pub adjacency_list: Vec<Vec<(usize, f64)>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SovResult {
    pub path: Vec<usize>,
    pub distance: f64,
}

#[command]
fn generate_tsp_data(n: usize, width: f64, height: f64) -> Result<TspData, String> {
    if n == 0 {
        return Err("Number of cities must be greater than 0".to_string());
    }

    let mut rng = rand::thread_rng();
    let mut cities = Vec::with_capacity(n);

    // 生成随机点坐标
    for i in 0..n {
        let x = rng.gen_range(0.0..width);
        let y = rng.gen_range(0.0..height);
        cities.push(City { id: i, x, y });
    }

    // 计算欧几里得距离矩阵（邻接表格式）
    let mut adjacency_list = Vec::with_capacity(n);

    for i in 0..n {
        let mut edges = Vec::with_capacity(n - 1);
        let (x1, y1) = (cities[i].x, cities[i].y);

        for j in 0..n {
            if i == j {
                continue;
            }

            let (x2, y2) = (cities[j].x, cities[j].y);
            let dx = x1 - x2;
            let dy = y1 - y2;
            let distance = (dx * dx + dy * dy).sqrt();
            let distance = (distance * 100.0).round() / 100.0;

            edges.push((j, distance));
        }

        adjacency_list.push(edges);
    }

    Ok(TspData {
        cities,
        adjacency_list,
    })
}

#[command]
fn solve_greedy(data: TspData, start_city: Option<usize>) -> Result<SovResult, String> {
    let path = greedy_tsp(&data.cities, &data.adjacency_list, start_city);
    let distance = calculate_path_distance(&path, &data.adjacency_list);

    Ok(SovResult { path, distance })
}

#[command]
fn solve_greedy_best(data: TspData) -> Result<SovResult, String> {
    let (path, distance) = greedy_tsp_best_start(&data.cities, &data.adjacency_list);

    Ok(SovResult { path, distance })
}

#[command]
fn solve_my_mst(data: TspData, start_city: Option<usize>) -> Result<SovResult, String> {
    let path = my_mst_tsp(&data.cities, &data.adjacency_list, start_city);
    let distance = calculate_path_distance(&path, &data.adjacency_list);

    Ok(SovResult { path, distance })
}


/* 核心内容 */
#[command]
fn python_file_save(code: String, file_path: String) -> Result<(), String> {
    use std::fs::File;
    use std::io::Write;

    let mut file = File::create(&file_path).map_err(|e| format!("Failed to create file: {}", e))?;
    file.write_all(code.as_bytes())
        .map_err(|e| format!("Failed to write to file: {}", e))?;

    Ok(())
}



#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            generate_tsp_data,
            solve_greedy,
            solve_greedy_best,
            solve_my_mst
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
