import numpy as np
import pandas as pd
from scipy.optimize import minimize    # 备用：SLSQP精修
from scipy.optimize import Bounds
# ====================== 第一问的物理常数 ======================
ALPHA = 6.327
K = np.array([79.636, 50.315, 132.613, 166.583])
GAMMA = np.array([0.00460, 0.00582, 0.00165, 0.00172])
# 振打和电压项的系数（用于C_out_pred）
A_COEF = K                    # U²/Q 项系数
B_COEF = -K * GAMMA           # U²T/Q 项系数


def c_out_pred(U, T, C_in, Q):
    """根据一问题模型预测出口浓度 (mg/Nm³)
    U, T: 长度为4的数组 (kV, s)
    C_in: 入口浓度 (g/Nm³)
    Q: 流量 (Nm³/h)
    """
    U = np.atleast_1d(U)
    T = np.atleast_1d(T)
    Y = ALPHA + np.sum(A_COEF * U**2 / Q) + np.sum(B_COEF * U**2 * T / Q)
    return C_in * 1000.0 / np.exp(Y)


def power_pred(U, T):
    """电耗预测 (kW)，使用第一问的回归系数"""
    coef = {
        'const': 1301.5693,
        'U1': 1.7915, 'U2': 2.0089, 'U3': 1.8968, 'U4': 2.1145,
        'T1': -1.0826, 'T2': -1.0781, 'T3': -0.2587, 'T4': -0.2564,
        'U1_sq': 0.0707, 'U2_sq': 0.0692, 'U3_sq': 0.0646, 'U4_sq': 0.0632
    }
    U_dict = {f'U{i+1}': U[i] for i in range(4)}
    T_dict = {f'T{i+1}': T[i] for i in range(4)}
    sq_dict = {f'U{i+1}_sq': U[i]**2 for i in range(4)}
    return (coef['const'] +
            sum(coef[k] * U_dict[k] for k in U_dict) +
            sum(coef[k] * T_dict[k] for k in T_dict) +
            sum(coef[k] * sq_dict[k] for k in sq_dict))


# ====================== PSO 核心（来自你的框架，已改写） ======================
def pso_optimize(bounds, C_in, Q, n_particles=80, max_iter=200,
                 stagnant_limit=50, penalty_weight=500.0):
    """
    最小化电耗，约束 C_out <= 10 (通过惩罚函数实现)
    bounds: 8维 (U1..U4, T1..T4) 的上、下界
    C_in, Q: 工况参数
    """
    dim = len(bounds)
    low = np.array([b[0] for b in bounds])
    high = np.array([b[1] for b in bounds])

    # --- 初始化 ---
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
        c_out = c_out_pred(U, T, C_in, Q)
        penalty = max(0, c_out - 5.0) * penalty_weight   # 超过5才惩罚
        val = power_pred(U, T) + penalty
        pbest_val[i] = val
        if val < gbest_val:
            gbest_val = val
            gbest_pos = pos[i].copy()
    """
    print(f"初始最优 电耗={power_pred(gbest_pos[:4], gbest_pos[4:8]):.1f} kW, "
          f"C_out={c_out_pred(gbest_pos[:4], gbest_pos[4:8], C_in, Q):.2f} mg/Nm³")
"""
    # PSO 参数
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

            # 评估
            U = pos[i][:4]
            T = pos[i][4:8]
            c_out = c_out_pred(U, T, C_in, Q)
            penalty = max(0, c_out - 5.0) * penalty_weight
            val = power_pred(U, T) + penalty

            if val < pbest_val[i]:
                pbest_val[i] = val
                pbest_pos[i] = pos[i].copy()
                if val < gbest_val:
                    gbest_val = val
                    gbest_pos = pos[i].copy()
                    stagnant = 0

        stagnant += 1
        if it % 30 == 0:
            best_U = gbest_pos[:4]
            best_T = gbest_pos[4:8]
            """
            print(f"Iter {it+1:3d} | 电耗={power_pred(best_U, best_T):.1f} kW | "
                  f"C_out={c_out_pred(best_U, best_T, C_in, Q):.2f} mg/Nm³")
            """         
        if stagnant > stagnant_limit:
            print(f"停滞，提前结束于迭代 {it+1}。")
            break

    return gbest_pos, gbest_val


# ====================== 第二问主流程 ======================
def main():
    # 读取数据 (获取参数边界)
    df = pd.read_csv(r"C:\Users\24070\Desktop\Cement_ESP_Data.csv")
    df = df.dropna(subset=['C_out_mgNm3'])

    # 工况划分（复用你之前的结果）
    from sklearn.cluster import KMeans
    from sklearn.preprocessing import StandardScaler
    feats = df[['C_in_gNm3','Temp_C']].values
    scaled = StandardScaler().fit_transform(feats)
    kmeans = KMeans(n_clusters=5, random_state=42, n_init=10)
    df['工况'] = kmeans.fit_predict(scaled)

    工况值 = df.groupby('工况').agg(
        C_in=('C_in_gNm3', 'median'),
        Q=('Q_Nm3h', 'median')
    )

    # 参数边界（从全数据提取，保证覆盖）
    bounds = [(df['U1_kV'].min(), df['U1_kV'].max()),
              (df['U2_kV'].min(), df['U2_kV'].max()),
              (df['U3_kV'].min(), df['U3_kV'].max()),
              (df['U4_kV'].min(), df['U4_kV'].max()),
              (60.0, df['T1_s'].max()),    # T下限60秒
              (60.0, df['T2_s'].max()),
              (60.0, df['T3_s'].max()),
              (60.0, df['T4_s'].max())]

    results = []
    for _, row in 工况值.iterrows():
        case = row.name
        c_in = row['C_in']
        q = row['Q']
        print(f"\n========== 工况 {case} | C_in={c_in:.1f} | Q={q:.0f} ==========")

        best_x, best_val = pso_optimize(bounds, c_in, q,
                                        n_particles=80, max_iter=200,
                                        penalty_weight=1000.0)
        U_opt = best_x[:4]
        T_opt = best_x[4:8]
        c_final = c_out_pred(U_opt, T_opt, c_in, q)
        p_final = power_pred(U_opt, T_opt)

        results.append({
            '工况': case,
            'C_in': c_in,
            'Q': q,
            'U1': U_opt[0], 'U2': U_opt[1], 'U3': U_opt[2], 'U4': U_opt[3],
            'T1': T_opt[0], 'T2': T_opt[1], 'T3': T_opt[2], 'T4': T_opt[3],
            '电耗': p_final,
            'C_out预测': c_final,
            '超标': c_final > 5 + 1e-6
        })
        print(f"最优: 电耗={p_final:.1f} kW, C_out={c_final:.2f} mg/Nm³")

    result_df = pd.DataFrame(results)
    print("\n========== 各工况最优汇总 ==========")
    print(result_df.to_string(index=False))
    result_df.to_csv(r"C:\Users\24070\Desktop\question4_PSO_results.csv", index=False)
    return result_df


# ====================== 多次运行统计 ======================
def run_pso_multiple(bounds, C_in, Q, n_runs=20):
    """独立运行PSO n_runs次，记录每次的最优参数和电耗"""
    results = []
    for run in range(n_runs):
        best_x, best_val = pso_optimize(bounds, C_in, Q, 
                                        n_particles=80, max_iter=200,
                                        penalty_weight=1000.0)
        U = best_x[:4]
        T = best_x[4:8]
        results.append({
            'run': run + 1,
            'U1': U[0], 'U2': U[1], 'U3': U[2], 'U4': U[3],
            'T1': T[0], 'T2': T[1], 'T3': T[2], 'T4': T[3],
            'power': best_val,
            'C_out': c_out_pred(U, T, C_in, Q)
        })
    return pd.DataFrame(results)

# ====================== 局部搜索确认 ======================
def local_refine(x0, C_in, Q, bounds_list):
    """以PSO结果为初始点，用SLSQP做局部精修"""
    def con_cout(x):
        U = x[:4]
        T = x[4:8]
        return 5.0 - c_out_pred(U, T, C_in, Q)
    
    cons = {'type': 'ineq', 'fun': con_cout}
    bounds = Bounds([b[0] for b in bounds_list], [b[1] for b in bounds_list])
    
    res = minimize(lambda x: power_pred(x[:4], x[4:8]), 
                   x0, method='SLSQP', bounds=bounds,
                   constraints=cons, options={'maxiter': 200, 'ftol': 1e-10})
    return res

# ====================== 主验证流程 ======================
def validate_optimization():
    df = pd.read_csv(r"C:\Users\24070\Desktop\Cement_ESP_Data.csv")
    df = df.dropna(subset=['C_out_mgNm3'])
    
    from sklearn.cluster import KMeans
    from sklearn.preprocessing import StandardScaler
    feats = df[['C_in_gNm3','Temp_C']].values
    scaled = StandardScaler().fit_transform(feats)
    kmeans = KMeans(n_clusters=5, random_state=42, n_init=10)
    df['工况'] = kmeans.fit_predict(scaled)
    
    工况值 = df.groupby('工况').agg(
        C_in=('C_in_gNm3', 'median'),
        Q=('Q_Nm3h', 'median')
    )
    
    bounds_list = [(df['U1_kV'].min(), df['U1_kV'].max()),
                   (df['U2_kV'].min(), df['U2_kV'].max()),
                   (df['U3_kV'].min(), df['U3_kV'].max()),
                   (df['U4_kV'].min(), df['U4_kV'].max()),
                   (60.0, df['T1_s'].max()),
                   (60.0, df['T2_s'].max()),
                   (60.0, df['T3_s'].max()),
                   (60.0, df['T4_s'].max())]
    
    print("="*80)
    print("验证报告：PSO求解可靠性分析")
    print("="*80)
    
    for _, row in 工况值.iterrows():
        case = row.name
        c_in = row['C_in']
        q = row['Q']
        print(f"\n{'='*60}")
        print(f"工况 {case} | C_in={c_in:.1f} | Q={q:.0f}")
        print(f"{'='*60}")
        
        # 1) 多次运行统计
        print(f"\n--- 独立运行20次统计 ---")
        runs = run_pso_multiple(bounds_list, c_in, q, n_runs=20)
        p_stats = runs['power'].describe()

        print(f"电耗: 均值={p_stats['mean']:.2f} kW, "
              f"标准差={p_stats['std']:.4f} kW, "
              f"最小值={p_stats['min']:.2f} kW, "
              f"最大值={p_stats['max']:.2f} kW")
        print(f"变异系数: {p_stats['std']/p_stats['mean']*100:.3f}%")

        # 判断收敛性
        cv = p_stats['std'] / p_stats['mean'] * 100
        if cv < 0.5:
            print("✓ 变异系数<0.5%，算法高度稳定")
        elif cv < 2.0:
            print("△ 变异系数<2%，算法基本稳定")
        else:
            print("✗ 变异系数>2%，算法不稳定，需增大粒子数或迭代次数")
        
        # C_out统计
        c_stats = runs['C_out'].describe()
        print(f"C_out: 均值={c_stats['mean']:.4f}, 全部满足约束={all(runs['C_out'] <= 10.01)}")
        
        # 2) 取最好解做局部精修
        best_run = runs.loc[runs['power'].idxmin()]
        x0 = np.array([best_run['U1'], best_run['U2'], best_run['U3'], best_run['U4'],
                       best_run['T1'], best_run['T2'], best_run['T3'], best_run['T4']])
        
        print(f"\n--- 局部梯度精修验证 ---")
        refine_res = local_refine(x0, c_in, q, bounds_list)
        if refine_res.success:
            U_ref = refine_res.x[:4]
            T_ref = refine_res.x[4:8]
            p_ref = refine_res.fun
            c_ref = c_out_pred(U_ref, T_ref, c_in, q)
            
            improvement = best_run['power'] - p_ref
            print(f"SLSQP精修后电耗: {p_ref:.2f} kW (原PSO: {best_run['power']:.2f} kW)")
            print(f"改善量: {improvement:.4f} kW")
            
            if abs(improvement) < 0.01:
                print("✓ PSO解已为局部最优（改善<0.01 kW），SLSQP无法进一步优化")
            elif improvement > 0.01:
                print("△ PSO解被SLSQP改善，原解非精确局部最优")
            else:
                print("（数值波动，可忽略）")
            
            print(f"C_out精修后: {c_ref:.6f} mg/Nm³")
            if c_ref <= 5.001:
                print("✓ 精修后仍满足约束")
            else:
                print("✗ 精修后违反约束！")
        else:
            print(f"SLSQP未收敛: {refine_res.message}")
    
    print(f"\n{'='*80}")
    print("验证完成")
    print("="*80)

if __name__ == "__main__":
    #main()
    validate_optimization()