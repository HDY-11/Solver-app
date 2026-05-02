import numpy as np

# ====================== 第一问中已有的辅助函数 ======================
def cylinder_surface_points(center, radius, height, density):
    """
    生成圆柱表面采样点，用于后续遮蔽率计算。
    center  : 底面圆心 (x, y, z)
    radius  : 半径
    height  : 高度
    density : 面密度（个/平方米），决定采样点数
    """
    cx, cy, cz = center
    side_area = 2 * np.pi * radius * height
    cap_area = np.pi * radius * radius
    side_n = max(1, int(side_area * density))
    cap_n = max(1, int(cap_area * density))

    # 侧面采样：在柱面上均匀分布
    frac = np.linspace(0, 1, side_n, endpoint=False)
    theta = frac * 2 * np.pi
    z = frac * height
    side = np.column_stack([cx + radius * np.cos(theta),
                            cy + radius * np.sin(theta),
                            cz + z])
    # 顶/底面采样：黄金角方法保证均匀
    golden = 2.399963
    idx = np.arange(cap_n)
    r = radius * np.sqrt(idx / cap_n)
    th = idx * golden
    dx = r * np.cos(th)
    dy = r * np.sin(th)
    bottom = np.column_stack([cx + dx, cy + dy, np.full(cap_n, cz)])
    top = np.column_stack([cx + dx, cy + dy, np.full(cap_n, cz + height)])
    return np.vstack([side, bottom, top])


def is_within_distance_batch(m1, sample_pts, ymd, d):
    """
    批量计算点 ymd 到线段 m1->sample_pts 的最短距离是否 ≤ d。
    m1         : 导弹位置 (3,)
    sample_pts : 圆柱表面采样点 (M, 3)
    ymd        : 烟幕球心位置 (3,)
    d          : 有效半径
    返回布尔数组 (M,)，True表示该视线被遮蔽。
    """
    v2 = sample_pts - m1                      # 线段方向向量
    v2_sq = np.sum(v2**2, axis=1)             # 长度平方
    degenerate = v2_sq < 1e-12                # 退化情况（m1=采样点）

    v1 = ymd - m1
    # 投影参数 t：m1 到垂足在线段上的比例
    t = np.where(degenerate, 0.0, np.dot(v2, v1) / v2_sq)

    dist_sq = np.empty(len(sample_pts))
    mask_before = t <= 0.0                    # 垂足在 m1 外侧
    mask_after  = t >= 1.0                    # 垂足在采样点外侧
    mask_mid    = ~(mask_before | mask_after) & ~degenerate  # 垂足在线段内

    # 最近点为 m1
    dist_sq[mask_before] = np.sum(v1**2)
    # 最近点为采样点
    dist_sq[mask_after]  = np.sum((ymd - sample_pts[mask_after])**2, axis=1)
    # 最近点为垂足
    cross = np.cross(v1, v2[mask_mid])
    dist_sq[mask_mid] = np.sum(cross**2, axis=1) / v2_sq[mask_mid]
    # 退化线段，直接计算到 m1 的距离
    dist_sq[degenerate] = np.sum(v1**2)

    return dist_sq <= d * d


# ====================== 全局常量 ======================
# 真目标（圆柱）的表面采样点，只生成一次，提高效率
_TARGET_POINTS = cylinder_surface_points((0.0, 200.0, 0.0), 7.0, 10.0, 0.1)
print(f"采样点数: {len(_TARGET_POINTS)}")

# 物理参数（与第一问完全一致，z轴向上为正）
_G = -9.8          # 重力加速度（负值，因为z向上）
_V_SINK = -3.0     # 云团下沉速度（负值，向下）

# 导弹参数
_M1_SPEED = 300.0                           # 导弹速度 (m/s)
_M1_INIT = np.array([20000.0, 0.0, 2000.0]) # 导弹初始位置
_M1_DIR = -_M1_INIT / np.linalg.norm(_M1_INIT)  # 导弹飞行方向，指向原点（假目标）

# 无人机初始位置
_FY1_INIT = np.array([17800.0, 0.0, 1800.0])

# 仿真参数
_DT = 0.01           # 时间步长 (s)
_RATIO_THRESHOLD = 0.95  # 有效遮蔽率阈值


# ====================== 遮蔽时间计算（核心目标函数） ======================
def compute_shading_duration(theta, v, t_d, delta_t):
    """
    根据决策变量计算有效遮蔽时长（秒）。
    theta  : 航向角（弧度），0为正东（x轴正向），π为正西（指向原点方向）
    v      : 无人机飞行速度 (m/s)，范围 [70, 140]
    t_d    : 从受领任务到投放的等待时间 (s)
    delta_t: 投放后到起爆的延时 (s)
    """
    # ---------- 1. 投放点计算 ----------
    # 无人机等高度匀速飞行
    x_d = _FY1_INIT[0] + v * np.cos(theta) * t_d
    y_d = _FY1_INIT[1] + v * np.sin(theta) * t_d
    z_d = _FY1_INIT[2]                     # 高度保持不变

    # ---------- 2. 起爆点计算 ----------
    # 烟幕弹自由落体，初速与无人机相同
    x_b = x_d + v * np.cos(theta) * delta_t
    y_b = y_d + v * np.sin(theta) * delta_t
    z_b = z_d + 0.5 * _G * delta_t**2      # z减小（向下运动）

    # ---------- 3. 时间边界 ----------
    t_hit = np.linalg.norm(_M1_INIT) / _M1_SPEED  # 导弹命中假目标的时间

    t_burst = t_d + delta_t                 # 起爆时刻
    if t_burst >= t_hit:                    # 还没起爆就命中，无效
        return 0.0

    t_end = min(t_burst + 20.0, t_hit)      # 有效时间上限：20秒后 或 命中时刻
    n_steps = max(0, int((t_end - t_burst) / _DT) + 1)
    if n_steps == 0:
        return 0.0

    # ---------- 4. 时间推进仿真 ----------
    t_arr = t_burst + np.arange(n_steps) * _DT  # 所有仿真时刻

    # 导弹位置矩阵 (n_steps, 3)
    m1_pos = _M1_INIT + _M1_DIR * (_M1_SPEED * t_arr[:, None])

    # 烟幕球心位置矩阵 (n_steps, 3)：水平固定，垂直匀速下沉
    ymd_pos = np.column_stack([
        np.full(n_steps, x_b),
        np.full(n_steps, y_b),
        z_b + _V_SINK * (t_arr - t_burst)
    ])

    # 逐帧判断遮蔽率
    effective = 0.0
    for step in range(n_steps):
        within = is_within_distance_batch(m1_pos[step],
                                          _TARGET_POINTS,
                                          ymd_pos[step],
                                          10.0)      # 10米有效半径
        if np.mean(within) >= _RATIO_THRESHOLD:     # 遮蔽率 ≥ 95%
            effective += _DT

    return effective


# ====================== 粒子群优化器 ======================
def pso_optimize(bounds, n_particles=50, max_iter=150, stagnant_limit=40,
                 seed_points=None):
    """
    标准粒子群算法（PSO），使用收缩因子，最大化遮蔽时长。
    bounds        : 各维度的 (下界, 上界) 列表
    n_particles   : 粒子数量
    max_iter      : 最大迭代代数
    stagnant_limit: 停滞代数上限
    seed_points   : 已知好解的列表，用于引导初始化
    """
    dim = len(bounds)
    low = np.array([b[0] for b in bounds])
    high = np.array([b[1] for b in bounds])

    # ---------- 智能初始化 ----------
    # 基于第一问已知解，把搜索范围限制在合理区域
    theta_center = np.pi                 # 180°，指向原点
    theta_spread = np.deg2rad(20)        # ±20°的搜索范围


    # 种子点 + 随机粒子混合
    if seed_points is not None:
        n_seeds = len(seed_points)
        n_rand = n_particles - n_seeds
        pos_seeds = np.array(seed_points)
        init_low = np.array([theta_center - theta_spread, 100, 0.5, 1.0])
        init_high = np.array([theta_center + theta_spread, 140, 6.0, 8.0])
        pos_rand = np.random.uniform(init_low, init_high, (n_rand, dim))
        pos = np.vstack([pos_seeds, pos_rand])
    else:
        init_low = np.array([theta_center - theta_spread, 100, 0.5, 1.0])
        init_high = np.array([theta_center + theta_spread, 140, 6.0, 8.0])
        pos = np.random.uniform(init_low, init_high, (n_particles, dim))

    vel = np.zeros((n_particles, dim))

    # 个体历史最优
    pbest_pos = pos.copy()
    pbest_val = np.full(n_particles, -np.inf)
    gbest_val = -np.inf
    gbest_pos = np.zeros(dim)

    # 评估初始种群
    for i in range(n_particles):
        val = compute_shading_duration(*pos[i])
        pbest_val[i] = val
        if val > gbest_val:
            gbest_val = val
            gbest_pos = pos[i].copy()
    print(f"初始最优: {gbest_val:.3f} s")

    # PSO 超参数
    v_max = 0.15 * (high - low)          # 速度限幅（各维范围的15%）
    chi, c1, c2 = 0.7298, 2.05, 2.05    # 收缩因子与加速常数
    stagnant = 0

    # 迭代主循环
    for it in range(max_iter):
        w = 0.9 - 0.5 * it / max_iter    # 惯性权重从0.9线性递减到0.4

        for i in range(n_particles):
            r1, r2 = np.random.rand(dim), np.random.rand(dim)
            # 速度更新公式（标准PSO）
            vel[i] = chi * (w * vel[i]
                            + c1 * r1 * (pbest_pos[i] - pos[i])
                            + c2 * r2 * (gbest_pos - pos[i]))
            vel[i] = np.clip(vel[i], -v_max, v_max)
            pos[i] += vel[i]

            # 边界处理：反射法，保证搜索不越界
            for d in range(dim):
                if pos[i][d] < low[d]:
                    pos[i][d] = low[d] + (low[d] - pos[i][d])
                    pos[i][d] = min(pos[i][d], high[d])
                    vel[i][d] *= -1
                elif pos[i][d] > high[d]:
                    pos[i][d] = high[d] - (pos[i][d] - high[d])
                    pos[i][d] = max(pos[i][d], low[d])
                    vel[i][d] *= -1

            # 评估新位置
            val = compute_shading_duration(*pos[i])
            if val > pbest_val[i]:       # 更新个体最优
                pbest_val[i] = val
                pbest_pos[i] = pos[i].copy()
                if val > gbest_val:      # 更新全局最优
                    gbest_val = val
                    gbest_pos = pos[i].copy()
                    stagnant = 0

        stagnant += 1
        if it % 20 == 0 or it == max_iter - 1:
            print(f"Iter {it+1:3d} | best = {gbest_val:.3f} s | "
                  f"param = {np.array2string(gbest_pos, precision=3)}")
        if stagnant > stagnant_limit:
            print(f"停滞于迭代 {it+1}，提前结束。")
            break

    return gbest_pos, gbest_val


# ====================== 问题2主函数 ======================
def main_2():
    """
    问题2：利用 FY1 投放 1 枚烟幕干扰弹，寻找最优的四元组参数
    (theta, v, t_d, delta_t) 使有效遮蔽时长最大。
    """

    # 决策变量边界
    bounds = [(0.0, 2 * np.pi), (70.0, 140.0), (0.0, 60.0), (0.0, 30.0)]

    # 注入第一问已知解作为种子，帮助算法快速定位
    seed_points = [[np.pi, 120.0, 1.5, 3.6]]

    # 执行粒子群优化
    best_params, best_time = pso_optimize(
        bounds,
        n_particles=50,
        max_iter=200,
        stagnant_limit=60,
        seed_points=seed_points
    )

    theta, v, t_d, delta_t = best_params

    print("\n========== 最优结果 ==========")
    print(f"航向角 theta   = {np.degrees(theta):.2f}° (弧度 {theta:.4f})")
    print(f"飞行速度 v      = {v:.2f} m/s")
    print(f"投放等待时间 t_d = {t_d:.2f} s")
    print(f"起爆延时 Δt     = {delta_t:.2f} s")
    print(f"最长有效遮蔽时长 = {best_time:.3f} s")

    # 计算并展示投放点、起爆点
    x_d = _FY1_INIT[0] + v * np.cos(theta) * t_d
    y_d = _FY1_INIT[1] + v * np.sin(theta) * t_d
    z_d = _FY1_INIT[2]
    x_b = x_d + v * np.cos(theta) * delta_t
    y_b = y_d + v * np.sin(theta) * delta_t
    z_b = z_d + 0.5 * _G * delta_t**2
    print(f"\n投放点: ({x_d:.1f}, {y_d:.1f}, {z_d:.1f})")
    print(f"起爆点: ({x_b:.1f}, {y_b:.1f}, {z_b:.1f})")

    return best_params, best_time


# ====================== 多次运行检验收敛性 ======================
def run_multiple_times(N=10):
    """
    独立运行 N 次 PSO 优化，统计最优值的分布，
    用以检验算法是否稳定收敛。
    """
    results = []
    for run in range(1, N + 1):
        print(f"\n{'='*20} 第 {run}/{N} 次运行 {'='*20}")
        seed_points = [[np.pi, 120.0, 1.5, 3.6]]
        bounds = [(0.0, 2 * np.pi), (70.0, 140.0), (0.0, 60.0), (0.0, 30.0)]
        _, best_time = pso_optimize(bounds, n_particles=50,
                                    max_iter=200, stagnant_limit=60,
                                    seed_points=seed_points)
        results.append(best_time)

    results = np.array(results)
    print(f"\n========== 多次运行统计 ==========")
    print(f"最优值: {results.max():.3f} s")
    print(f"平均值: {results.mean():.3f} s")
    print(f"标准差: {results.std():.3f} s")
    print(f"全部结果: {np.round(results, 3)}")
    return results


if __name__ == "__main__":
    main_2()