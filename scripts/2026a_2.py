import pandas as pd
import numpy as np
from scipy.optimize import minimize, Bounds, NonlinearConstraint
from sklearn.cluster import KMeans
from sklearn.preprocessing import StandardScaler
import statsmodels.api as sm
import warnings
warnings.filterwarnings('ignore')

# ============================================================
# 0. 读取数据
# ============================================================
df = pd.read_csv(r"C:\Users\24070\Desktop\Cement_ESP_Data.csv")
# 确保时间戳列存在，但不用于聚类
df = df.dropna(subset=['C_in_gNm3', 'Temp_C', 'Q_Nm3h',
                        'U1_kV','U2_kV','U3_kV','U4_kV',
                        'T1_s','T2_s','T3_s','T4_s',
                        'P_total_kW'])

# ============================================================
# 1. 工况划分：基于 C_in 和 Temp 的 K-means
# ============================================================
cluster_vars = ['C_in_gNm3', 'Temp_C']
X = df[cluster_vars].values

# 标准化（K-means 需要）
scaler = StandardScaler()
X_scaled = scaler.fit_transform(X)

# 确定聚类数：尝试 3,4,5，选轮廓系数最大的
from sklearn.metrics import silhouette_score
best_k = 4
best_score = -1
for k in [3,4,5]:
    km = KMeans(n_clusters=k, random_state=42, n_init=10)
    labels = km.fit_predict(X_scaled)
    score = silhouette_score(X_scaled, labels)
    if score > best_score:
        best_score = score
        best_k = k

# 最终聚类
kmeans = KMeans(n_clusters=best_k, random_state=42, n_init=10)
df['工况'] = kmeans.fit_predict(X_scaled)

# 计算每个工况的 C_in 和 Q 的中位数（用于优化）
工况代表值 = df.groupby('工况').agg(
    C_in_median=('C_in_gNm3', 'median'),
    Q_median=('Q_Nm3h', 'median'),
    Temp_median=('Temp_C', 'median')
).reset_index()

# ============================================================
# 2. 电耗预测模型 P_total = f(U1,U2,U3,U4, T1,T2,T3,T4)
# ============================================================
# 准备自变量：尝试线性项和平方项
U_vars = ['U1_kV','U2_kV','U3_kV','U4_kV']
T_vars = ['T1_s','T2_s','T3_s','T4_s']

# 线性项
X_lin = df[U_vars + T_vars].copy()
# 平方项（电压平方，振打周期取对数？这里保守一点也用平方项）
for v in U_vars:
    X_lin[f'{v}_sq'] = df[v] ** 2
# 注意：也可以加 T 的平方，但物理意义不强，先不加

# 添加常数项
X_lin = sm.add_constant(X_lin)
y = df['P_total_kW']

# 用 OLS 拟合全模型
model_lin = sm.OLS(y, X_lin).fit()

# 简化模型：用逐步选择（AIC 最小）——这里用后向消除
# 为稳健，保留所有主效应，若高 VIF 再处理
# 我们直接使用全模型的预测，因为预测能力强即可
P_model = model_lin

# 提取参数：用于优化目标函数
coeffs = P_model.params
intercept = coeffs['const']
# 构建系数字典，方便按变量名取值
coef_dict = coeffs.to_dict()

print(f"电耗模型 R² = {P_model.rsquared:.4f}")

# ============================================================
# 3. 物理模型常数（来自第一问，排除50.00那版的结果）
# ============================================================
alpha = 6.327
K = np.array([79.636, 50.315, 132.613, 166.583])   # K1..K4
gamma = np.array([0.00460, 0.00582, 0.00165, 0.00172])  # γ1..γ4
b_coef = -0.366, -0.293, -0.219, -0.286   # 用于 U^2 T / Q 项的系数 b_i（负值）
a_coef = 79.636, 50.315, 132.613, 166.583  # U^2/Q 项系数

def c_out_pred(U, T, C_in, Q):
    """预测出口浓度 (mg/Nm3)，U 和 T 为长度为4的数组"""
    U = np.array(U)
    T = np.array(T)
    # 计算对数效率 Y_target
    Y = alpha + np.sum(a_coef * U**2 / Q) + np.sum(b_coef * U**2 * T / Q)
    return C_in * 1000 / np.exp(Y)

# ============================================================
# 4. 定义优化问题
# ============================================================
# 参数边界：从数据中获取
bounds_list = []
for col in ['U1_kV','U2_kV','U3_kV','U4_kV']:
    bounds_list.append((df[col].min(), df[col].max()))
for col in ['T1_s','T2_s','T3_s','T4_s']:
    bounds_list.append((max(60, df[col].min()), df[col].max()))  # 振打周期不低于60秒

bounds = Bounds([b[0] for b in bounds_list], [b[1] for b in bounds_list])

def objective(x):
    """电耗预测值"""
    U1,U2,U3,U4 = x[0], x[1], x[2], x[3]
    T1,T2,T3,T4 = x[4], x[5], x[6], x[7]
    # 构建与回归模型相同的特征向量
    # 注意顺序必须与模型训练时的列一致
    features = {
        'const': 1.0,
        'U1_kV': U1, 'U2_kV': U2, 'U3_kV': U3, 'U4_kV': U4,
        'T1_s': T1, 'T2_s': T2, 'T3_s': T3, 'T4_s': T4,
        'U1_kV_sq': U1**2, 'U2_kV_sq': U2**2, 'U3_kV_sq': U3**2, 'U4_kV_sq': U4**2
    }
    pred = intercept
    for name, val in features.items():
        pred += coef_dict.get(name, 0.0) * val
    return pred

# 优化函数（每个工况）
def optimize_for_case(c_in, q):
    # 约束：c_out_pred <= 10
    def con_cout(x):
        U = x[:4]
        T = x[4:8]
        return 10 - c_out_pred(U, T, c_in, q)  # ≥0 表示满足
    nl_cons = NonlinearConstraint(con_cout, 0, np.inf)

    # 初始猜测：取数据中位数
    x0 = np.array([
        df['U1_kV'].median(), df['U2_kV'].median(), df['U3_kV'].median(), df['U4_kV'].median(),
        df['T1_s'].median(), df['T2_s'].median(), df['T3_s'].median(), df['T4_s'].median()
    ])

    res = minimize(objective, x0, method='SLSQP', bounds=bounds,
                   constraints=nl_cons, options={'maxiter': 500, 'ftol': 1e-8})
    return res

# ============================================================
# 5. 对各工况优化并汇总
# ============================================================
results = []

for _, row in 工况代表值.iterrows():
    case_id = int(row['工况'])
    c_in = row['C_in_median']
    q = row['Q_median']
    temp = row['Temp_median']

    res = optimize_for_case(c_in, q)
    if res.success:
        U_opt = res.x[:4]
        T_opt = res.x[4:8]
        p_opt = res.fun
        c_out_opt = c_out_pred(U_opt, T_opt, c_in, q)
        results.append({
            '工况': case_id,
            'C_in (g/Nm3)': c_in,
            'Q (Nm3/h)': q,
            'Temp (C)': temp,
            'U1 (kV)': U_opt[0], 'U2 (kV)': U_opt[1], 'U3 (kV)': U_opt[2], 'U4 (kV)': U_opt[3],
            'T1 (s)': T_opt[0], 'T2 (s)': T_opt[1], 'T3 (s)': T_opt[2], 'T4 (s)': T_opt[3],
            '电耗 (kW)': p_opt,
            '出口浓度预测 (mg/Nm3)': c_out_opt
        })
    else:
        results.append({'工况': case_id, '备注': f'优化失败: {res.message}'})

result_df = pd.DataFrame(results)

# ============================================================
# 6. 输出报告
# ============================================================
output_path = r"C:\Users\24070\Desktop\question2_report.txt"
with open(output_path, 'w', encoding='utf-8') as f:
    f.write("="*80 + "\n")
    f.write("第二问：工况划分与协同优化结果\n")
    f.write("="*80 + "\n\n")
    f.write(f"工况划分方法：K-means (k={best_k})，基于入口浓度和温度\n")
    f.write(f"轮廓系数 = {best_score:.4f}\n\n")
    f.write("各工况代表值（中位数）：\n")
    f.write(工况代表值.to_string(index=False) + "\n\n")
    f.write("--- 电耗预测模型 ---\n")
    f.write(f"R² = {P_model.rsquared:.4f}\n")
    f.write(str(P_model.summary()) + "\n\n")
    f.write("--- 优化结果（出口浓度约束 ≤ 10 mg/Nm3） ---\n")
    f.write(result_df.to_string(index=False) + "\n\n")
    f.write("说明：优化目标为电耗最小；出口浓度由第一问多依奇模型预测。\n")
    f.write("振打周期下限设为60秒。\n")

print(f"报告已保存至: {output_path}")
print("优化结果预览：")
print(result_df)