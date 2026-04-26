import numpy as np

def cylinder_surface_points(center, radius, height, density):
    """与 Rust simple_cylinder_surface 完全一致"""
    cx, cy, cz = center
    side_area = 2 * np.pi * radius * height
    cap_area = np.pi * radius * radius
    side_n = max(1, int(side_area * density))
    cap_n = max(1, int(cap_area * density))

    # 侧面
    frac = np.linspace(0, 1, side_n, endpoint=False)
    theta = frac * 2 * np.pi
    z = frac * height
    side = np.column_stack([cx + radius * np.cos(theta),
                            cy + radius * np.sin(theta),
                            cz + z])

    # 顶/底面（黄金角）
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
    对应 Rust 的 is_within_distance(&m1, s, &ymd, d)
    判断点 ymd 到每个线段 m1->s 的距离是否 ≤ d。
    返回布尔数组 (len(sample_pts),)
    """
    v2 = sample_pts - m1                      # (M,3)  线段方向
    v2_sq = np.sum(v2**2, axis=1)            # (M,)

    # 退化线段 (端点重合)
    degenerate = v2_sq < 1e-12
    v1 = ymd - m1                              # (3,)
    t = np.where(degenerate, 0.0, np.dot(v2, v1) / v2_sq)

    dist_sq = np.empty(len(sample_pts))
    mask_before = t <= 0.0
    mask_after  = t >= 1.0
    mask_mid    = ~(mask_before | mask_after) & ~degenerate

    # t <= 0：最近点是 m1
    dist_sq[mask_before] = np.sum(v1**2)   # 常量
    # t >= 1：最近点是 s
    dist_sq[mask_after]  = np.sum((ymd - sample_pts[mask_after])**2, axis=1)
    # 0 < t < 1：垂线距离
    cross = np.cross(v1, v2[mask_mid])          # (n_mid,3)
    dist_sq[mask_mid] = np.sum(cross**2, axis=1) / v2_sq[mask_mid]
    # 退化段同样用 m1 到 ymd 的距离
    dist_sq[degenerate] = np.sum(v1**2)

    return dist_sq <= d * d


def main():
    DT = 0.01
    TOTAL_STEPS = 2000   # 20 s
    T0 = 5.1

    # ----- 质点初始化 -----
    m1_init = np.array([20000.0, 0.0, 2000.0])
    direction = -m1_init
    direction /= np.linalg.norm(direction)
    m1_v = direction * 300.0
    # m1 在 5.1 s 内运动（update 已执行）
    m1_pos = m1_init + m1_v * T0

    # ymd：初始位置就是 5.1 s 时的位置，没有提前位移！
    ymd_init = np.array([17188.0, 0.0, 1736.496])
    ymd_v = np.array([0.0, 0.0, -3.0])
    ymd_pos = ymd_init.copy()

    # ----- 烟幕圆柱采样 -----
    sample_pts = cylinder_surface_points((0.0, 200.0, 0.0), 7.0, 10.0, 0.1)
    N = len(sample_pts)
    print(f"采样点数: {N}")

    effective_time = 0.0
    current_time = T0

    for step in range(TOTAL_STEPS):
        m1_pos += m1_v * DT
        ymd_pos += ymd_v * DT
        current_time += DT

        # 计算遮蔽率
        within = is_within_distance_batch(m1_pos, sample_pts, ymd_pos, 10.0)
        ratio = np.mean(within)

        if ratio > 0.95:
            effective_time += DT

        if step % 100 == 0 or step == TOTAL_STEPS - 1:
            print(f"[{current_time:.2f}s] M1:({m1_pos[0]:.1f},{m1_pos[1]:.1f},{m1_pos[2]:.1f}) "
                  f"YMD:({ymd_pos[0]:.1f},{ymd_pos[1]:.1f},{ymd_pos[2]:.1f}) "
                  f"有效:{effective_time:.3f}s 遮蔽率:{ratio*100:.2f}%")

    print(f"\n总有效遮蔽时长 = {effective_time:.3f}s")


def main_2_1():
    # 首先，确定一个空间，在这个空间中，fy1投放有意义
    # 怎么算有意义？
    # fy1在xoy,xoz,yoz上的投影，在m1，ft中间
    # 分别向6个正方向以最大速度飞，每隔0.01s判断一次。误差不小，但可以经过后续调整消除，关键是保证解空间包含最优解

    # 0时刻
    DT = 0.01

    fy1 = np.array([17800.0, 0.0, 1800.0], dtype=np.float64)
    m1 = np.array([20000.0, 0.0, 2000.0], dtype=np.float64)
    ft = np.array([0.0, 200.0, 0.0], dtype=np.float64)

    v_max_val = 140.0

    ans_space = np.zeros((6,3), dtype=np.float64)
    vec_l = [
        np.array([1.0, 0.0, 0.0], dtype=np.float64),
        np.array([-1.0, 0.0, 0.0], dtype=np.float64),
        np.array([0.0, 1.0, 0.0], dtype=np.float64),
        np.array([0.0, -1.0, 0.0], dtype=np.float64),
        np.array([0.0, 0.0, 1.0], dtype=np.float64),
        np.array([0.0, 0.0, -1.0], dtype=np.float64)
    ]
    for i in range(0,6):
        v_max = v_max_val*vec_l[i]
        for dt in range(0,2000):
            if ft[0] >= fy1[0] >= m1[0] or ft[1] >= fy1[1] >= m1[1] or ft[2] >= fy1[2] >= m1[2]:
                ans_space[i] = fy1
                break
            


    return 0


if __name__ == "__main__":
    main()