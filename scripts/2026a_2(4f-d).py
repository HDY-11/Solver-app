import numpy as np
import pandas as pd
from sklearn.cluster import KMeans
from sklearn.preprocessing import StandardScaler
from datetime import datetime
import warnings
warnings.filterwarnings('ignore')

# ======================== 全局物理常数（第一问结果） ========================
ALPHA = 6.327
K = np.array([79.636, 50.315, 132.613, 166.583])
GAMMA = np.array([0.00460, 0.00582, 0.00165, 0.00172])
B_COEF = -K * GAMMA           # b_i = -K_i * gamma_i

# ======================== 电耗回归模型系数 ========================
P_COEF = {
    'const': 1301.5693,
    'U1': 1.7915, 'U2': 2.0089, 'U3': 1.8968, 'U4': 2.1145,
    'T1': -1.0826, 'T2': -1.0781, 'T3': -0.2587, 'T4': -0.2564,
    'U1_sq': 0.0707, 'U2_sq': 0.0692, 'U3_sq': 0.0646, 'U4_sq': 0.0632
}

# ======================== 惩罚系数基准 ========================
LAM_U_BASE = 50.0      # 电压超参考范围惩罚（二次项系数）
LAM_T_BASE = 5.0       # 振打过短惩罚（倒数项系数）
LAM_L_BASE = 30.0      # 电压过低惩罚（二次项系数）
T_CRIT = 180.0          # 振打临界周期（倒数惩罚起点）

# ======================== 优化参数 ========================
C_OUT_LIMIT = 10.0      # 排放标准 (mg/Nm³)
PENALTY_WEIGHT = 1000.0 # 超标惩罚系数
N_PARTICLES = 80
MAX_ITER = 200
N_RUNS = 20             # 每组实验独立运行次数（取最优）

# ======================== 数据路径 ========================
DATA_PATH = r"C:\Users\24070\Desktop\Cement_ESP_Data.csv"

# ======================== 预测函数 ========================
def c_out_pred(U, T, C_in, Q):
    """出口浓度预测 (mg/Nm³)"""
    U = np.atleast_1d(U)
    T = np.atleast_1d(T)
    Y = ALPHA + np.sum(K * U**2 / Q) + np.sum(B_COEF * U**2 * T / Q)
    return C_in * 1000.0 / np.exp(Y)

def power_pred(U, T):
    """电耗预测 (kW)"""
    return (P_COEF['const'] +
            P_COEF['U1']*U[0] + P_COEF['U2']*U[1] + P_COEF['U3']*U[2] + P_COEF['U4']*U[3] +
            P_COEF['T1']*T[0] + P_COEF['T2']*T[1] + P_COEF['T3']*T[2] + P_COEF['T4']*T[3] +
            P_COEF['U1_sq']*U[0]**2 + P_COEF['U2_sq']*U[1]**2 + P_COEF['U3_sq']*U[2]**2 + P_COEF['U4_sq']*U[3]**2)

# ======================== 外推惩罚（物理惩罚） ========================
def extrapolation_penalty(U, T, ref_U_max, ref_T_min,
                          lam_U=LAM_U_BASE, lam_T=LAM_T_BASE, lam_L=LAM_L_BASE):
    """
    当参数超出参考范围时施加物理惩罚
    ref_U_max : 参考电压上限（如数据最大值或国标上限）
    ref_T_min : 参考振打下限（实际惩罚由 T_CRIT 触发，此处保留接口）
    """
    penalty = 0.0
    # 电压超限惩罚：二次型，系数 lam_U
    for i in range(4):
        if U[i] > ref_U_max[i]:
            penalty += lam_U * (U[i] - ref_U_max[i])**2
        if U[i] < 40.0:   # 电压过低通用惩罚
            penalty += lam_L * (40.0 - U[i])**2
    # 振打过短惩罚：倒数型，系数 lam_T
    for i in range(4):
        if T[i] < T_CRIT:
            penalty += lam_T / max(T[i] - T_CRIT, 5.0)   # 避免除零
    return penalty

# ======================== PSO 优化器 ========================
def pso_optimize(bounds, C_in, Q, mode='min_power',
                 ref_U_max=None, ref_T_min=None,
                 lam_U=LAM_U_BASE, lam_T=LAM_T_BASE, lam_L=LAM_L_BASE):
    dim = len(bounds)
    low = np.array([b[0] for b in bounds], dtype=float)
    high = np.array([b[1] for b in bounds], dtype=float)
    np.random.seed(42)
    pos = np.random.uniform(low, high, (N_PARTICLES, dim))
    vel = np.zeros((N_PARTICLES, dim))
    pbest_pos = pos.copy()
    pbest_val = np.full(N_PARTICLES, np.inf)
    gbest_val = np.inf
    gbest_pos = np.zeros(dim)

    # 评估初始种群
    for i in range(N_PARTICLES):
        U, T = pos[i][:4], pos[i][4:8]
        c_out = c_out_pred(U, T, C_in, Q)
        if mode == 'min_power':
            penalty_cout = max(0, c_out - C_OUT_LIMIT) * PENALTY_WEIGHT
            penalty_extrap = 0.0
            if ref_U_max is not None:
                penalty_extrap = extrapolation_penalty(U, T, ref_U_max, ref_T_min,
                                                       lam_U, lam_T, lam_L)
            val = power_pred(U, T) + penalty_cout + penalty_extrap
        else:  # min_cout
            val = c_out
        pbest_val[i] = val
        if val < gbest_val:
            gbest_val = val
            gbest_pos = pos[i].copy()

    v_max = 0.15 * (high - low)
    chi, c1, c2 = 0.7298, 2.05, 2.05
    stagnant = 0
    for it in range(MAX_ITER):
        w = 0.9 - 0.5 * it / MAX_ITER
        for i in range(N_PARTICLES):
            r1, r2 = np.random.rand(dim), np.random.rand(dim)
            vel[i] = chi * (w * vel[i] + c1*r1*(pbest_pos[i]-pos[i]) + c2*r2*(gbest_pos-pos[i]))
            vel[i] = np.clip(vel[i], -v_max, v_max)
            pos[i] += vel[i]
            for d in range(dim):
                if pos[i][d] < low[d]:
                    pos[i][d] = 2*low[d] - pos[i][d]
                    if pos[i][d] > high[d]: pos[i][d] = high[d]
                    vel[i][d] *= -0.5
                elif pos[i][d] > high[d]:
                    pos[i][d] = 2*high[d] - pos[i][d]
                    if pos[i][d] < low[d]: pos[i][d] = low[d]
                    vel[i][d] *= -0.5
            U, T = pos[i][:4], pos[i][4:8]
            c_out = c_out_pred(U, T, C_in, Q)
            if mode == 'min_power':
                penalty_cout = max(0, c_out - C_OUT_LIMIT) * PENALTY_WEIGHT
                penalty_extrap = 0.0
                if ref_U_max is not None:
                    penalty_extrap = extrapolation_penalty(U, T, ref_U_max, ref_T_min,
                                                           lam_U, lam_T, lam_L)
                val = power_pred(U, T) + penalty_cout + penalty_extrap
            else:
                val = c_out
            if val < pbest_val[i]:
                pbest_val[i] = val
                pbest_pos[i] = pos[i].copy()
                if val < gbest_val:
                    gbest_val = val
                    gbest_pos = pos[i].copy()
                    stagnant = 0
        stagnant += 1
        if stagnant > 50:
            break
    return gbest_pos, gbest_val

# ======================== 工况划分 ========================
def get_cases(df):
    feats = StandardScaler().fit_transform(df[['C_in_gNm3','Temp_C']].values)
    df['工况'] = KMeans(n_clusters=5, random_state=42, n_init=10).fit_predict(feats)
    return df.groupby('工况').agg(C_in=('C_in_gNm3', 'median'), Q=('Q_Nm3h', 'median'))

# ======================== 主实验 ========================
def main():
    df = pd.read_csv(DATA_PATH).dropna(subset=['C_out_mgNm3'])
    cases = get_cases(df)

    # 数据范围
    U_data_max = [df['U1_kV'].max(), df['U2_kV'].max(), df['U3_kV'].max(), df['U4_kV'].max()]
    U_data_min = [df['U1_kV'].min(), df['U2_kV'].min(), df['U3_kV'].min(), df['U4_kV'].min()]
    T_data_max = [df['T1_s'].max(), df['T2_s'].max(), df['T3_s'].max(), df['T4_s'].max()]
    T_data_min = [df['T1_s'].min(), df['T2_s'].min(), df['T3_s'].min(), df['T4_s'].min()]

    # 定义边界
    # 习惯组边界（数据极值，T下限210s）
    habit_bounds = [(U_data_min[0], U_data_max[0]), (U_data_min[1], U_data_max[1]),
                    (U_data_min[2], U_data_max[2]), (U_data_min[3], U_data_max[3]),
                    (210.0, T_data_max[0]), (210.0, T_data_max[1]),
                    (210.0, T_data_max[2]), (210.0, T_data_max[3])]

    # 国标组边界（40-72 kV，T≥210s）
    ns_bounds = [(40.0, 72.0), (40.0, 72.0), (40.0, 72.0), (40.0, 72.0),
                 (210.0, T_data_max[0]), (210.0, T_data_max[1]),
                 (210.0, T_data_max[2]), (210.0, T_data_max[3])]

    # 理想组边界（40-90 kV，T≥130s）
    ideal_bounds = [(40.0, 90.0), (40.0, 90.0), (40.0, 90.0), (40.0, 90.0),
                    (130.0, T_data_max[0]), (130.0, T_data_max[1]),
                    (130.0, T_data_max[2]), (130.0, T_data_max[3])]

    # 惩罚参考（习惯范围 = 数据极值，国标范围 = 72 kV 上限）
    ref_U_habit = U_data_max                # 习惯电压上限参考
    ref_U_ns = [72.0, 72.0, 72.0, 72.0]    # 国标电压上限参考
    ref_T_any = [210.0]*4                   # 振打参考（实际惩罚不依赖）

    # 六组实验定义
    experiments = [
        # (名称, 搜索边界, 参考电压上限, 参考振打下限, 是否开启惩罚)
        ('Habit', habit_bounds, None, None, False),
        ('NS', ns_bounds, None, None, False),
        ('Ideal', ideal_bounds, None, None, False),
        ('Habit2NS', ns_bounds, ref_U_habit, ref_T_any, True),        # 搜索边界国标，惩罚参照习惯电压上限
        ('Habit2Ideal', ideal_bounds, ref_U_habit, ref_T_any, True),  # 搜索边界理想，惩罚参照习惯电压上限
        ('NS2Ideal', ideal_bounds, ref_U_ns, ref_T_any, True)         # 搜索边界理想，惩罚参照国标电压上限
    ]

    log_path = r"C:\Users\24070\Desktop\optimization_full_log.txt"
    with open(log_path, 'w', encoding='utf-8') as f:
        f.write("="*80 + "\n")
        f.write("  六组实验（含 min_power / min_cout）\n")
        f.write(f"  运行时间: {datetime.now()}\n")
        f.write("="*80 + "\n\n")

        all_results = []

        for group_name, bounds, ref_U, ref_T, use_penalty in experiments:
            for mode in ['min_power', 'min_cout']:
                print(f"\n====== {group_name} | {mode} ======")
                f.write(f"\n====== {group_name} | {mode} ======\n")
                for case, row in cases.iterrows():
                    C_in, Q = row['C_in'], row['Q']
                    best_val = np.inf
                    best_x = None
                    for _ in range(N_RUNS):
                        x, val = pso_optimize(bounds, C_in, Q, mode=mode,
                                              ref_U_max=ref_U if use_penalty else None,
                                              ref_T_min=ref_T if use_penalty else None)
                        if val < best_val:
                            best_val = val
                            best_x = x
                    U_opt, T_opt = best_x[:4], best_x[4:8]
                    c_final = c_out_pred(U_opt, T_opt, C_in, Q)
                    p_final = power_pred(U_opt, T_opt)
                    feasible = c_final <= C_OUT_LIMIT + 1e-6

                    all_results.append({
                        '实验组': group_name,
                        '模式': mode,
                        '工况': case,
                        'C_in': C_in, 'Q': Q,
                        'U1': U_opt[0], 'U2': U_opt[1], 'U3': U_opt[2], 'U4': U_opt[3],
                        'T1': T_opt[0], 'T2': T_opt[1], 'T3': T_opt[2], 'T4': T_opt[3],
                        '电耗_kW': p_final,
                        'C_out': c_final,
                        '达标': feasible
                    })
                    status = '达标' if feasible else '超标'
                    print(f"  工况{case}: C_out={c_final:.2f}, 电耗={p_final:.1f}kW, {status}")
                    f.write(f"  工况{case}: C_out={c_final:.2f}, 电耗={p_final:.1f}kW, {status}\n")

        result_df = pd.DataFrame(all_results)
        result_df.to_csv(r"C:\Users\24070\Desktop\full_results.csv", index=False)

        # ================== 灵敏度分析 ==================
        print("\n====== 灵敏度分析（惩罚系数 ±50%） ======")
        f.write("\n\n====== 灵敏度分析 ======\n")
        sens_factors = [0.5, 0.75, 1.0, 1.25, 1.5]
        rep_cases = [0, 4]   # 高浓度工况0 和 低浓度工况4
        sens_results = []

        # 只对三个惩罚组进行灵敏度分析
        penalty_groups = [e for e in experiments if e[4]]  # use_penalty == True
        for group_name, bounds, ref_U, ref_T, _ in penalty_groups:
            for coeff_name, base_val in [('lam_U', LAM_U_BASE), ('lam_T', LAM_T_BASE), ('lam_L', LAM_L_BASE)]:
                for factor in sens_factors:
                    coeff_val = base_val * factor
                    lam_U_cur = coeff_val if coeff_name == 'lam_U' else LAM_U_BASE
                    lam_T_cur = coeff_val if coeff_name == 'lam_T' else LAM_T_BASE
                    lam_L_cur = coeff_val if coeff_name == 'lam_L' else LAM_L_BASE
                    for case in rep_cases:
                        C_in = cases.loc[case, 'C_in']
                        Q = cases.loc[case, 'Q']
                        best_val = np.inf
                        best_x = None
                        # 灵敏度分析用 5 次运行加快速度
                        for _ in range(5):
                            x, val = pso_optimize(bounds, C_in, Q, mode='min_power',
                                                  ref_U_max=ref_U, ref_T_min=ref_T,
                                                  lam_U=lam_U_cur, lam_T=lam_T_cur, lam_L=lam_L_cur)
                            if val < best_val:
                                best_val = val
                                best_x = x
                        U_opt, T_opt = best_x[:4], best_x[4:8]
                        c_final = c_out_pred(U_opt, T_opt, C_in, Q)
                        p_final = power_pred(U_opt, T_opt)
                        sens_results.append({
                            '惩罚组': group_name,
                            '系数名': coeff_name,
                            '因子': factor,
                            '系数值': coeff_val,
                            '工况': case,
                            '电耗_kW': p_final,
                            'C_out': c_final,
                            '达标': c_final <= C_OUT_LIMIT + 1e-6
                        })
        sens_df = pd.DataFrame(sens_results)
        sens_df.to_csv(r"C:\Users\24070\Desktop\sensitivity_analysis.csv", index=False)
        print("灵敏度分析完成，结果已保存。")
        f.write("灵敏度分析完成。\n")

if __name__ == "__main__":
    main()