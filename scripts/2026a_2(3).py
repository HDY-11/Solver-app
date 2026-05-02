import numpy as np
import pandas as pd
from scipy.optimize import minimize, Bounds
from sklearn.cluster import KMeans
from sklearn.preprocessing import StandardScaler

# ====================== 第一问物理常数 ======================
ALPHA = 6.327
K = np.array([79.636, 50.315, 132.613, 166.583])
GAMMA = np.array([0.00460, 0.00582, 0.00165, 0.00172])
B_COEF = -K * GAMMA

# ====================== 出口浓度预测（带 η_total） ======================
def c_out_pred_with_eta(U, T, C_in, Q, eta_total=1.0):
    U = np.atleast_1d(U)
    T = np.atleast_1d(T)
    Y_ideal = ALPHA + np.sum(K * U**2 / Q) + np.sum(B_COEF * U**2 * T / Q)
    Y_eff = eta_total * Y_ideal
    return C_in * 1000.0 / np.exp(Y_eff)

# ====================== 电耗预测 ======================
def power_pred(U, T):
    return (1301.5693 +
            1.7915*U[0] + 2.0089*U[1] + 1.8968*U[2] + 2.1145*U[3] +
            -1.0826*T[0] - 1.0781*T[1] - 0.2587*T[2] - 0.2564*T[3] +
            0.0707*U[0]**2 + 0.0692*U[1]**2 + 0.0646*U[2]**2 + 0.0632*U[3]**2)

# ====================== PSO 优化（带 η_total） ======================
def pso_optimize_eta(bounds, C_in, Q, eta_total, n_particles=80, max_iter=200, penalty_weight=1000.0):
    dim = len(bounds)
    low = np.array([b[0] for b in bounds], dtype=float)
    high = np.array([b[1] for b in bounds], dtype=float)

    pos = np.random.uniform(low, high, (n_particles, dim))
    vel = np.zeros((n_particles, dim))
    pbest_pos = pos.copy()
    pbest_val = np.full(n_particles, np.inf)
    gbest_val = np.inf
    gbest_pos = np.zeros(dim)

    # 评估初始种群
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

            # 边界反射
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

# ====================== 主验证流程（整合新约束） ======================
def validate_with_literature_constraints():
    df = pd.read_csv(r"C:\Users\24070\Desktop\Cement_ESP_Data.csv").dropna(subset=['C_out_mgNm3'])
    feats = StandardScaler().fit_transform(df[['C_in_gNm3','Temp_C']].values)
    df['工况'] = KMeans(n_clusters=5, random_state=42, n_init=10).fit_predict(feats)
    
    工况值 = df.groupby('工况').agg(C_in=('C_in_gNm3', 'median'), Q=('Q_Nm3h', 'median'))

    # 基于文献的硬约束
    U_min, U_max = 40.0, 72.0   # kV
    T_min, T_max = 210.0, df['T1_s'].max() # 秒
    
    bounds_list = [(U_min, U_max), (U_min, U_max), (U_min, U_max), (U_min, U_max),
                   (T_min, T_max), (T_min, T_max), (T_min, T_max), (T_min, T_max)]

    eta_scenarios = [0.8, 0.9, 1.0]
    all_results = []

    print("="*80)
    print("验证报告：引入文献约束与粉尘修正后的PSO求解")
    print("="*80)
    
    for eta in eta_scenarios:
        print(f"\n========== 场景: η_total = {eta} ==========")
        for _, row in 工况值.iterrows():
            case, C_in, Q = row.name, row['C_in'], row['Q']
            print(f"\n--- 工况 {case} | C_in={C_in:.1f} | Q={Q:.0f} ---")
            
            best_x, best_val = pso_optimize_eta(bounds_list, C_in, Q, eta)
            U_opt, T_opt = best_x[:4], best_x[4:8]
            p_final = power_pred(U_opt, T_opt)
            c_final = c_out_pred_with_eta(U_opt, T_opt, C_in, Q, eta)
            feasible = c_final <= 10.01
            
            all_results.append({
                'η_total': eta, '工况': case, 'C_in': C_in, 'Q': Q,
                'U1': U_opt[0], 'U2': U_opt[1], 'U3': U_opt[2], 'U4': U_opt[3],
                'T1': T_opt[0], 'T2': T_opt[1], 'T3': T_opt[2], 'T4': T_opt[3],
                '电耗': p_final, 'C_out': c_final, '可行': feasible
            })
            
            status = "✓ 可行" if feasible else "✗ 不可行"
            print(f"  最优: 电耗={p_final:.1f} kW, C_out={c_final:.2f} mg/Nm³ {status}")

    result_df = pd.DataFrame(all_results)
    result_df.to_csv(r"C:\Users\24070\Desktop\final_optimization_with_literature.csv", index=False)
    
    print("\n\n==================== 汇总对比 ====================")
    print(result_df.to_string(index=False))

if __name__ == "__main__":
    validate_with_literature_constraints()