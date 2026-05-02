import pandas as pd
import numpy as np
from scipy.optimize import minimize, Bounds
from sklearn.cluster import KMeans
from sklearn.preprocessing import StandardScaler

# ====================== 物理常数 ======================
ALPHA = 6.327
K = np.array([79.636, 50.315, 132.613, 166.583])
GAMMA = np.array([0.00460, 0.00582, 0.00165, 0.00172])
B_COEF = -K * GAMMA   # b_i

# ====================== 修正后的出口浓度预测 ======================
def c_out_pred_with_eta(U, T, C_in, Q, eta_total=1.0):
    """
    eta_total: 综合修正因子 (0, 1]，用于表示二次扬尘、反电晕等损失。
    """
    U = np.atleast_1d(U)
    T = np.atleast_1d(T)
    # 理想对数效率
    Y_ideal = ALPHA + np.sum(K * U**2 / Q) + np.sum(B_COEF * U**2 * T / Q)
    # 实际有效对数效率
    Y_eff = eta_total * Y_ideal
    return C_in * 1000.0 / np.exp(Y_eff)

# ====================== 电耗预测（不变） ======================
def power_pred(U, T):
    coef = {
        'const': 1301.5693,
        'U1': 1.7915, 'U2': 2.0089, 'U3': 1.8968, 'U4': 2.1145,
        'T1': -1.0826, 'T2': -1.0781, 'T3': -0.2587, 'T4': -0.2564,
        'U1_sq': 0.0707, 'U2_sq': 0.0692, 'U3_sq': 0.0646, 'U4_sq': 0.0632
    }
    return (coef['const'] +
            coef['U1']*U[0] + coef['U2']*U[1] + coef['U3']*U[2] + coef['U4']*U[3] +
            coef['T1']*T[0] + coef['T2']*T[1] + coef['T3']*T[2] + coef['T4']*T[3] +
            coef['U1_sq']*U[0]**2 + coef['U2_sq']*U[1]**2 + coef['U3_sq']*U[2]**2 + coef['U4_sq']*U[3]**2)

# ====================== PSO 优化（带 eta） ======================
def pso_optimize_eta(bounds, C_in, Q, eta_total, n_particles=80, max_iter=200, penalty_weight=1000.0):
    dim = len(bounds)
    low = np.array([b[0] for b in bounds])
    high = np.array([b[1] for b in bounds])
    
    pos = np.random.uniform(low, high, (n_particles, dim))
    vel = np.zeros((n_particles, dim))
    pbest_pos = pos.copy()
    pbest_val = np.full(n_particles, np.inf)
    gbest_val = np.inf
    gbest_pos = np.zeros(dim)

    for i in range(n_particles):
        U = pos[i][:4]
        T = pos[i][4:8]
        c_out = c_out_pred_with_eta(U, T, C_in, Q, eta_total)
        penalty = max(0, c_out - 10.0) * penalty_weight
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

            U = pos[i][:4]
            T = pos[i][4:8]
            c_out = c_out_pred_with_eta(U, T, C_in, Q, eta_total)
            penalty = max(0, c_out - 10.0) * penalty_weight
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

# ====================== 主分析流程 ======================
def main():
    df = pd.read_csv(r"C:\Users\24070\Desktop\Cement_ESP_Data.csv")
    df = df.dropna(subset=['C_out_mgNm3'])

    # 工况划分
    feats = df[['C_in_gNm3','Temp_C']].values
    scaled = StandardScaler().fit_transform(feats)
    kmeans = KMeans(n_clusters=5, random_state=42, n_init=10)
    df['工况'] = kmeans.fit_predict(scaled)
    工况值 = df.groupby('工况').agg(
        C_in=('C_in_gNm3', 'median'),
        Q=('Q_Nm3h', 'median')
    )

    # 参数边界
    bounds = [(df['U1_kV'].min(), df['U1_kV'].max()),
              (df['U2_kV'].min(), df['U2_kV'].max()),
              (df['U3_kV'].min(), df['U3_kV'].max()),
              (df['U4_kV'].min(), df['U4_kV'].max()),
              (60.0, df['T1_s'].max()),
              (60.0, df['T2_s'].max()),
              (60.0, df['T3_s'].max()),
              (60.0, df['T4_s'].max())]

    eta_values = [1.0, 0.9, 0.8, 0.7, 0.6, 0.5]
    all_results = []

    for eta in eta_values:
        print(f"\n========== η_total = {eta} ==========")
        for _, row in 工况值.iterrows():
            case = row.name
            c_in = row['C_in']
            q = row['Q']
            best_x, best_val = pso_optimize_eta(bounds, c_in, q, eta,
                                                n_particles=60, max_iter=150,
                                                penalty_weight=1000.0)
            U_opt = best_x[:4]
            T_opt = best_x[4:8]
            c_final = c_out_pred_with_eta(U_opt, T_opt, c_in, q, eta)
            p_final = power_pred(U_opt, T_opt)
            feasible = c_final <= 10.01

            all_results.append({
                'η_total': eta,
                '工况': case,
                'C_in': c_in,
                'Q': q,
                'U1': U_opt[0], 'U2': U_opt[1], 'U3': U_opt[2], 'U4': U_opt[3],
                'T1': T_opt[0], 'T2': T_opt[1], 'T3': T_opt[2], 'T4': T_opt[3],
                '电耗': p_final,
                'C_out': c_final,
                '可行': feasible
            })
            status = "✓ 可行" if feasible else "✗ 不可行"
            print(f"  工况 {case}: 电耗={p_final:.1f} kW, C_out={c_final:.2f} mg/Nm³ {status}")

    result_df = pd.DataFrame(all_results)
    result_df.to_csv(r"C:\Users\24070\Desktop\eta_sensitivity_results.csv", index=False)

    # 输出关键对比表
    print("\n\n==================== 汇总对比 ====================")
    pivot_power = result_df.pivot_table(values='电耗', index='工况', columns='η_total')
    pivot_cout = result_df.pivot_table(values='C_out', index='工况', columns='η_total')
    print("各工况最优电耗 (kW):")
    print(pivot_power)
    print("\n各工况出口浓度 (mg/Nm³):")
    print(pivot_cout)

    # 检查不可行情况
    infeasible = result_df[~result_df['可行']]
    if len(infeasible) > 0:
        print(f"\n⚠️ 不可行的工况 (η_total 过低导致参数边界内无法达标):")
        print(infeasible[['η_total','工况','C_in','C_out']])
    else:
        print("\n✓ 所有 η_total 组合均能找到满足 C_out≤10 的解。")

if __name__ == "__main__":
    main()