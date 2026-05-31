import numpy as np
import pandas as pd
from sklearn.cluster import KMeans
from sklearn.preprocessing import StandardScaler
from datetime import datetime
import warnings
warnings.filterwarnings('ignore')

# ==================== 物理常数 ====================
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

def extrapolation_penalty_physical(U, T, U_data_max, T_data_min):
    penalty = 0.0
    # 电压：指数型，轻微越界惩罚小，大幅越界惩罚急剧增大
    for i in range(4):
        if U[i] > U_data_max[i]:
            excess = U[i] - U_data_max[i]
            penalty += 10.0 * (np.exp(0.5 * excess) - 1)  # 参数可调
        if U[i] < 40.0:
            penalty += 10.0 * (np.exp(0.5 * (40.0 - U[i])) - 1)
    # 振打周期：倒数型，短于临界值惩罚急剧增大
    T_crit = 200.0  # 二次扬尘恶化临界值
    for i in range(4):
        if T[i] < T_crit:
            penalty += 2.0 / max(T[i] - 180.0, 5.0)  # 避免除零
    return penalty

def pso_optimize(bounds, C_in, Q, penalty_weight=1000.0,
                 mode='min_power', extrap_penalty=False,
                 U_data_max=None, T_data_min=None):
    dim = len(bounds)
    low = np.array([b[0] for b in bounds], dtype=float)
    high = np.array([b[1] for b in bounds], dtype=float)
    np.random.seed(42)
    pos = np.random.uniform(low, high, (80, dim))
    vel = np.zeros((80, dim))
    pbest_pos = pos.copy()
    pbest_val = np.full(80, np.inf)
    gbest_val = np.inf
    gbest_pos = np.zeros(dim)

    for i in range(80):
        U, T = pos[i][:4], pos[i][4:8]
        c_out = c_out_pred(U, T, C_in, Q)
        if mode == 'min_power':
            penalty = max(0, c_out - 10.0) * penalty_weight
            if extrap_penalty and U_data_max is not None:
                penalty += extrapolation_penalty_physical(U, T, U_data_max, T_data_min)
            val = power_pred(U, T) + penalty
        else:  # min_cout
            val = c_out
        pbest_val[i] = val
        if val < gbest_val:
            gbest_val = val
            gbest_pos = pos[i].copy()

    v_max = 0.15 * (high - low)
    chi, c1, c2 = 0.7298, 2.05, 2.05
    stagnant = 0
    for it in range(200):
        w = 0.9 - 0.5 * it / 200
        for i in range(80):
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
                penalty = max(0, c_out - 10.0) * penalty_weight
                if extrap_penalty and U_data_max is not None:
                    penalty += extrapolation_penalty_physical(U, T, U_data_max, T_data_min)
                val = power_pred(U, T) + penalty
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

# ==================== 主实验框架 ====================
def run_full_experiment():
    df = pd.read_csv(r"C:\Users\24070\Desktop\Cement_ESP_Data.csv").dropna(subset=['C_out_mgNm3'])
    feats = StandardScaler().fit_transform(df[['C_in_gNm3','Temp_C']].values)
    df['工况'] = KMeans(n_clusters=5, random_state=42, n_init=10).fit_predict(feats)
    工况值 = df.groupby('工况').agg(C_in=('C_in_gNm3', 'median'), Q=('Q_Nm3h', 'median'))

    U_data_max = [df['U1_kV'].max(), df['U2_kV'].max(), df['U3_kV'].max(), df['U4_kV'].max()]
    U_data_min = [df['U1_kV'].min(), df['U2_kV'].min(), df['U3_kV'].min(), df['U4_kV'].min()]
    T_data_max = [df['T1_s'].max(), df['T2_s'].max(), df['T3_s'].max(), df['T4_s'].max()]
    T_data_min = [df['T1_s'].min(), df['T2_s'].min(), df['T3_s'].min(), df['T4_s'].min()]

    # 三组边界定义
    bounds_A = [(U_data_min[0], U_data_max[0]), (U_data_min[1], U_data_max[1]),
                (U_data_min[2], U_data_max[2]), (U_data_min[3], U_data_max[3]),
                (210.0, T_data_max[0]), (210.0, T_data_max[1]),
                (210.0, T_data_max[2]), (210.0, T_data_max[3])]
    bounds_B = [(40.0, 72.0), (40.0, 72.0), (40.0, 72.0), (40.0, 72.0),
                (210.0, T_data_max[0]), (210.0, T_data_max[1]),
                (210.0, T_data_max[2]), (210.0, T_data_max[3])]
    # 实验 C 边界同 B，但开启外推惩罚

    log_path = r"C:\Users\24070\Desktop\optimization_MultiBound_log.txt"
    with open(log_path, 'w', encoding='utf-8') as f:
        f.write("="*80 + "\n")
        f.write("  多边界/多模式 PSO 优化实验\n")
        f.write(f"  运行时间: {datetime.now()}\n")
        f.write("="*80 + "\n\n")

        all_results = []
        for exp_id, (exp_name, bounds, use_extrap) in enumerate([
            ('数据极值边界', bounds_A, False),
            ('国标/行标边界', bounds_B, False),
            ('国标边界+外推惩罚', bounds_B, True)]):
            for mode in ['min_power', 'min_cout']:
                desc = f"{exp_name} | {mode}"
                print(f"\n{'='*60}\n实验 {exp_id+1}.{mode}  {desc}\n{'='*60}")
                f.write(f"\n{'='*60}\n实验 {exp_id+1}.{mode}  {desc}\n{'='*60}\n")
                
                for _, row in 工况值.iterrows():
                    case, C_in, Q = row.name, row['C_in'], row['Q']
                    # 多次运行取最优
                    best_val = np.inf
                    best_x = None
                    for _ in range(20):
                        x, val = pso_optimize(bounds, C_in, Q,
                                            mode=mode,
                                            extrap_penalty=use_extrap,
                                            U_data_max=U_data_max if use_extrap else None,
                                            T_data_min=T_data_min if use_extrap else None)
                        if val < best_val:
                            best_val = val
                            best_x = x
                    U_opt, T_opt = best_x[:4], best_x[4:8]
                    c_final = c_out_pred(U_opt, T_opt, C_in, Q)
                    p_final = power_pred(U_opt, T_opt)
                    
                    # 生成超越范围提示
                    beyond = []
                    for i in range(4):
                        if U_opt[i] > U_data_max[i] + 0.1: beyond.append(f"U{i+1}↑{U_opt[i]:.1f}>{U_data_max[i]:.1f}")
                        if U_opt[i] < U_data_min[i] - 0.1: beyond.append(f"U{i+1}↓{U_opt[i]:.1f}<{U_data_min[i]:.1f}")
                        if T_opt[i] < T_data_min[i] - 0.1: beyond.append(f"T{i+1}↓{T_opt[i]:.1f}<{T_data_min[i]:.1f}")
                    beyond_str = "; ".join(beyond) if beyond else "无"
                    
                    line = (f"工况{case}: U={U_opt.round(1)} T={T_opt.round(1)} | "
                            f"电耗={p_final:.1f}kW C_out={c_final:.2f}mg/Nm³")
                    if beyond_str != "无":
                        line += f"   ⚠ {beyond_str}"
                    print(line)
                    f.write(line + "\n")
                    
                    all_results.append({
                        '实验': exp_id+1, '边界描述': exp_name, '模式': mode,
                        '工况': case, 'C_in': C_in, 'Q': Q,
                        'U1': U_opt[0], 'U2': U_opt[1], 'U3': U_opt[2], 'U4': U_opt[3],
                        'T1': T_opt[0], 'T2': T_opt[1], 'T3': T_opt[2], 'T4': T_opt[3],
                        '电耗_kW': p_final, 'C_out': c_final,
                        '超出数据范围': beyond_str
                    })

        # 汇总表格
        result_df = pd.DataFrame(all_results)
        f.write("\n\n" + "="*80 + "\n  汇总对比表\n" + "="*80 + "\n")
        summary = result_df.pivot_table(index=['边界描述','模式','工况'],
                                        values=['电耗_kW','C_out'],
                                        aggfunc='first').round(2)
        f.write(summary.to_string())
        result_df.to_csv(r"C:\Users\24070\Desktop\optimization_MultiBound_results.csv", index=False)
        print(f"\n完整结果已保存至 {log_path} 和 CSV 文件")

if __name__ == "__main__":
    run_full_experiment()