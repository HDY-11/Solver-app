import numpy as np
import pandas as pd
from sklearn.cluster import KMeans
from sklearn.preprocessing import StandardScaler
import warnings
warnings.filterwarnings('ignore')

# ==================== 物理常数（同前） ====================
ALPHA = 6.327
K = np.array([79.636, 50.315, 132.613, 166.583])
GAMMA = np.array([0.00460, 0.00582, 0.00165, 0.00172])
B_COEF = -K * GAMMA

def c_out_pred(U, T, C_in, Q):
    U, T = np.atleast_1d(U), np.atleast_1d(T)
    Y = ALPHA + np.sum(K * U**2 / Q) + np.sum(B_COEF * U**2 * T / Q)
    return C_in * 1000.0 / np.exp(Y)

def power_pred(U, T):
    return (1301.5693 +
            1.7915*U[0] + 2.0089*U[1] + 1.8968*U[2] + 2.1145*U[3] +
            -1.0826*T[0] - 1.0781*T[1] - 0.2587*T[2] - 0.2564*T[3] +
            0.0707*U[0]**2 + 0.0692*U[1]**2 + 0.0646*U[2]**2 + 0.0632*U[3]**2)

# ==================== 外推惩罚函数（参数化） ====================
def extrapolation_penalty_param(U, T, U_data_max, T_data_min,
                                lam_U=50.0, lam_T=5.0, lam_L=30.0):
    penalty = 0.0
    for i in range(4):
        if U[i] > U_data_max[i]:
            penalty += lam_U * (U[i] - U_data_max[i])**2
        if U[i] < 40.0:
            penalty += lam_L * (40.0 - U[i])**2
    T_crit = 180.0
    for i in range(4):
        if T[i] < T_crit:
            penalty += lam_T / max(T[i] - T_crit, 5.0)  # 避免除零
    return penalty

# ==================== PSO（可传入惩罚系数） ====================
def pso_optimize_sensitivity(bounds, C_in, Q, penalty_weight=1000.0,
                             lam_U=50.0, lam_T=5.0, lam_L=30.0,
                             U_data_max=None, T_data_min=None,
                             n_particles=60, max_iter=150):
    dim = len(bounds)
    low = np.array([b[0] for b in bounds], dtype=float)
    high = np.array([b[1] for b in bounds], dtype=float)
    np.random.seed(42)
    pos = np.random.uniform(low, high, (n_particles, dim))
    vel = np.zeros((n_particles, dim))
    pbest_pos = pos.copy()
    pbest_val = np.full(n_particles, np.inf)
    gbest_val = np.inf
    gbest_pos = np.zeros(dim)

    for i in range(n_particles):
        U, T = pos[i][:4], pos[i][4:8]
        c_out = c_out_pred(U, T, C_in, Q)
        penalty = max(0, c_out - 10.0) * penalty_weight
        if U_data_max is not None:
            penalty += extrapolation_penalty_param(U, T, U_data_max, T_data_min,
                                                   lam_U, lam_T, lam_L)
        val = power_pred(U, T) + penalty
        pbest_val[i] = val
        if val < gbest_val:
            gbest_val = val
            gbest_pos = pos[i].copy()

    v_max = 0.15 * (high - low)
    chi, c1, c2 = 0.7298, 2.05, 2.05
    stagnant = 0
    for it in range(max_iter):
        w = 0.9 - 0.5 * it / max_iter
        for i in range(n_particles):
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
            penalty = max(0, c_out - 10.0) * penalty_weight
            if U_data_max is not None:
                penalty += extrapolation_penalty_param(U, T, U_data_max, T_data_min,
                                                       lam_U, lam_T, lam_L)
            val = power_pred(U, T) + penalty
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

# ==================== 主灵敏度分析 ====================
def sensitivity_analysis():
    # 数据准备
    df = pd.read_csv(r"C:\Users\24070\Desktop\Cement_ESP_Data.csv").dropna(subset=['C_out_mgNm3'])
    feats = StandardScaler().fit_transform(df[['C_in_gNm3','Temp_C']].values)
    df['工况'] = KMeans(n_clusters=5, random_state=42, n_init=10).fit_predict(feats)
    工况值 = df.groupby('工况').agg(C_in=('C_in_gNm3', 'median'), Q=('Q_Nm3h', 'median'))

    # 数据范围
    U_data_max = [df['U1_kV'].max(), df['U2_kV'].max(), df['U3_kV'].max(), df['U4_kV'].max()]
    T_data_min = [df['T1_s'].min(), df['T2_s'].min(), df['T3_s'].min(), df['T4_s'].min()]

    # 国标边界（实验C）
    bounds_std = [(40.0, 72.0)]*4 + [(210.0, df['T1_s'].max()), (210.0, df['T2_s'].max()),
                                      (210.0, df['T3_s'].max()), (210.0, df['T4_s'].max())]

    # 基准系数
    base = {'lam_U': 50.0, 'lam_T': 5.0, 'lam_L': 30.0}
    # 因子
    factors = [0.5, 0.75, 1.0, 1.25, 1.5]

    # 结果收集
    all_rows = []

    # 对每个系数单独变化，保持其他为基准
    for coeff_name in ['lam_U', 'lam_T', 'lam_L']:
        for factor in factors:
            # 构造当前系数
            current_coeff = base.copy()
            current_coeff[coeff_name] = base[coeff_name] * factor

            print(f"\n=== 系数 {coeff_name} × {factor} (={current_coeff[coeff_name]:.1f}) ===")
            for _, row in 工况值.iterrows():
                case, C_in, Q = row.name, row['C_in'], row['Q']
                # 简化版：只运行3次取最优，提高速度
                best_val = np.inf
                best_x = None
                for _ in range(3):
                    x, val = pso_optimize_sensitivity(bounds_std, C_in, Q,
                                                      lam_U=current_coeff['lam_U'],
                                                      lam_T=current_coeff['lam_T'],
                                                      lam_L=current_coeff['lam_L'],
                                                      U_data_max=U_data_max,
                                                      T_data_min=T_data_min,
                                                      n_particles=60, max_iter=150)
                    if val < best_val:
                        best_val = val
                        best_x = x
                U_opt, T_opt = best_x[:4], best_x[4:8]
                c_final = c_out_pred(U_opt, T_opt, C_in, Q)
                p_final = power_pred(U_opt, T_opt)
                feasible = c_final <= 10.01
                status = '✓达标' if feasible else '✗超标'
                all_rows.append({
                    '系数名': coeff_name,
                    '因子': factor,
                    '系数值': current_coeff[coeff_name],
                    '工况': case,
                    'C_in': C_in,
                    'Q': Q,
                    '电耗_kW': p_final,
                    'C_out': c_final,
                    '达标': feasible
                })
                print(f"  工况{case}: C_out={c_final:.2f}, 电耗={p_final:.1f}, {status}")

    # 转DataFrame并保存
    df_res = pd.DataFrame(all_rows)
    df_res.to_csv(r"C:\Users\24070\Desktop\sensitivity_analysis.csv", index=False)

    # 输出汇总：每个系数因子下，高浓和低浓的代表性表现
    print("\n\n==================== 汇总 ====================")
    summary = df_res.pivot_table(index=['系数名','因子'], columns='工况',
                                 values=['C_out','达标'], aggfunc='first')
    print(summary.to_string())

if __name__ == "__main__":
    sensitivity_analysis()