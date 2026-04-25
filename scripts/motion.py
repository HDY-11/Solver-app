import math
from simple_motion import Particle3, Motion


def main():
    # 初始化模拟系统，并设置起始时间为 5.1 s
    sim = Motion()
    sim.set_time(5.1)

    # ----- 构建粒子 m1（导弹） -----
    # 计算速度：从原点指向 (20000, 0, 2000) 的单位向量，乘以 300 并取反
    dx, dy, dz = 20000.0, 0.0, 2000.0
    dist = math.hypot(dx, dy, dz)          # 等效 sqrt(dx² + dy² + dz²)
    ux, uy, uz = dx / dist, dy / dist, dz / dist
    vx, vy, vz = -300.0 * ux, -300.0 * uy, -300.0 * uz

    # m1 初始位置，并直接计算 5.1 s 后的位置（匀速运动）
    init_pos = (20000.0, 0.0, 2000.0)
    pos_after_5_1 = (
        init_pos[0] + vx * 5.1,
        init_pos[1] + vy * 5.1,
        init_pos[2] + vz * 5.1,
    )

    # 创建不可变的 m1 粒子（位置、速度）
    m1 = Particle3(*pos_after_5_1).set_velocity(vx, vy, vz)

    # ----- 构建粒子 ymd（烟幕弹） -----
    ymd = (
        Particle3(17188.0, 0.0, 1736.496)
        .set_velocity(0.0, 0.0, -3.0)
        .set_lastest_time(5.1)
    )

    # 将两个粒子加入系统
    sim.add_particle(m1)
    sim.add_particle(ymd)

    # ----- 生成圆柱表面采样点（烟幕遮蔽体） -----
    # 圆柱底面中心 (0, 200, 0)，半径 7 m，高 10 m，密度 0.1 点/m²
    sample_pts = Motion.cylinder_surface_points(0.0, 200.0, 0.0, 7.0, 10.0, 0.1)

    # ----- 从 5.1 s 开始，模拟 20 s（2000 × 0.01 s） -----
    DT = 0.01
    effective_time = 0.0   # 满足遮蔽条件的累计时间

    for _ in range(2000):
        sim.update(DT)

        # 获取当前所有粒子的位置
        positions = sim.get_positions()
        m1_pos = tuple(positions[0])
        ymd_pos = tuple(positions[1])

        # 计算遮蔽率（点-线段距离 < 10 m 的比例）
        ratio = Motion.occlusion_ratio(m1_pos, ymd_pos, sample_pts, 10.0)

        if ratio > 0.95:
            effective_time += DT

        print(
            f"[{sim.time:.2f}s 时刻] M1位置: {m1_pos}, "
            f"烟幕圆心位置: {ymd_pos}, "
            f"遮蔽时长: {effective_time:.3f}, "
            f"遮蔽率: {ratio * 100:.2f}%"
        )


if __name__ == "__main__":
    main()