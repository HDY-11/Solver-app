import pandas as pd
import numpy as np
from scipy import stats

# ============================================================
# 1. 读取数据
# ============================================================
df = pd.read_csv(r"C:\Users\24070\Desktop\parsed.csv", parse_dates=['timestamp'])
df.sort_values('timestamp', inplace=True)
df.reset_index(drop=True, inplace=True)

# ============================================================
# 2. 回归得到的物理常数（排除50.00的结果）
# ============================================================
K = {1: 79.636, 2: 50.315, 3: 132.613, 4: 166.583}
gamma = {1: 0.00460, 2: 0.00582, 3: 0.00165, 4: 0.00172}

# 振打尘饼通过出口的时间（秒），典型值10-30秒
tau = 20  # 可取10-30

# ============================================================
# 3. 计算各电场捕集效率和积灰质量
# ============================================================
for i in range(1, 5):
    # 分级效率
    df[f'eta_{i}'] = 1 - np.exp(-K[i] * df[f'U{i}_kV']**2 / df['Q_Nm3h'])
    # 单次振打脱落的尘饼质量 (g)
    df[f'm_dust_{i}'] = df['C_in_gNm3'] * df[f'eta_{i}'] * df['Q_Nm3h'] * df[f'T{i}_s'] / 3600
    # 瞬时浓度峰值贡献 (mg/Nm3)：质量 / (流量 * 通过时间)
    df[f'delta_C_peak_{i}'] = (df[f'm_dust_{i}'] * 1000) / (df['Q_Nm3h'] * tau / 3600)
    # 简化：delta_C_peak = C_in * eta * T / tau
    df[f'delta_C_peak_simple_{i}'] = df['C_in_gNm3'] * df[f'eta_{i}'] * df[f'T{i}_s'] / tau

# ============================================================
# 4. 各电场峰值贡献统计
# ============================================================
print("=" * 60)
print("各电场振打瞬时峰值贡献统计 (mg/Nm³)")
print("=" * 60)
for i in range(1, 5):
    col = f'delta_C_peak_simple_{i}'
    print(f"\n电场 E{i}:")
    print(f"  均值: {df[col].mean():.2f}")
    print(f"  中位数: {df[col].median():.2f}")
    print(f"  最大值: {df[col].max():.2f}")
    print(f"  标准差: {df[col].std():.2f}")

# ============================================================
# 5. 最坏情况瞬时峰值（各电场最大值）
# ============================================================
# 假设稳态出口浓度 = 中位数（约50）
C_steady = df['C_out_mgNm3'].median()
# 各电场最坏情况叠加
worst_peaks = {}
for i in range(1, 5):
    worst_peaks[i] = df[f'delta_C_peak_simple_{i}'].max()
    
print("\n" + "=" * 60)
print(f"最坏情况瞬时峰值估算 (假设稳态={C_steady:.1f} mg/Nm³)")
print("=" * 60)
print(f"各电场单独最坏情况:")
for i in range(1, 5):
    print(f"  E{i}: {C_steady + worst_peaks[i]:.1f} mg/Nm³ (增加 {worst_peaks[i]:.1f})")

# ============================================================
# 6. 振打周期与峰值的关系
# ============================================================
print("\n" + "=" * 60)
print("振打周期对瞬时峰值的影响 (取 eta 中位数时的灵敏度)")
print("=" * 60)
for i in range(1, 5):
    eta_med = df[f'eta_{i}'].median()
    C_in_med = df['C_in_gNm3'].median()
    sensitivity = C_in_med * eta_med / tau  # 每增加1秒振打周期, 峰值增加量
    T_med = df[f'T{i}_s'].median()
    print(f"  E{i}: d(ΔC_peak)/dT = {sensitivity:.4f} mg/Nm³/s, "
          f"当前中位T={T_med:.0f}s, 峰值贡献中位={sensitivity*T_med:.1f} mg/Nm³")

# ============================================================
# 7. 物理推导的核心关系式输出
# ============================================================
print("\n" + "=" * 60)
print("第一问核心公式：瞬时峰值与操作参数的关系")
print("=" * 60)
print("""
ΔC_peak,i = C_in × η_i × T_i / τ

其中:
  η_i = 1 - exp(-K_i × U_i² / Q)
  K_i: 捕集系数 (E1=79.6, E2=50.3, E3=132.6, E4=166.6)
  τ: 尘饼团块通过时间 (取20s)
  T_i: 振打周期
  
结论: 瞬时峰值 ∝ T_i × C_in × η_i
""")

# ============================================================
# 8. 残差验证：物理推导 vs 数据痕迹
# ============================================================
print("=" * 60)
print("残差验证：出口浓度微小波动是否与振打扰动指标相关")
print("=" * 60)

# 移动中位数平滑趋势
df['C_smooth'] = df['C_out_mgNm3'].rolling(window=60, center=True).median()
df['C_resid_abs'] = (df['C_out_mgNm3'] - df['C_smooth']).abs()

# 扰动指标
for i in range(1, 5):
    df[f'disturb_{i}'] = df['C_in_gNm3'] * df[f'eta_{i}'] * df[f'T{i}_s'] / df['Q_Nm3h']

# 回归
valid = df[['C_resid_abs', 'disturb_1', 'disturb_2', 'disturb_3', 'disturb_4']].dropna()
X = valid[['disturb_1', 'disturb_2', 'disturb_3', 'disturb_4']]
y = valid['C_resid_abs']

# 手动最小二乘
X_with_const = np.column_stack([np.ones(len(X)), X.values])
beta, residuals, rank, s = np.linalg.lstsq(X_with_const, y.values, rcond=None)

print(f"\n残差幅度 ~ Σ β_i × disturb_i")
print(f"R² = {1 - residuals[0] / np.sum((y - y.mean())**2):.4f}")
print(f"\n系数:")
print(f"  截距: {beta[0]:.4f}")
for i in range(1, 5):
    print(f"  disturb_{i}: {beta[i]:.6f}")

# t检验
n, k = len(X), 4
sigma2 = residuals[0] / (n - k - 1)
XtX_inv = np.linalg.inv(X_with_const.T @ X_with_const)
se = np.sqrt(np.diag(XtX_inv) * sigma2)
t_stats = beta / se
p_values = 2 * (1 - stats.t.cdf(np.abs(t_stats), n - k - 1))

print(f"\n显著性检验:")
print(f"  截距: t={t_stats[0]:.2f}, p={p_values[0]:.4f}")
for i in range(1, 5):
    sig = "***" if p_values[i] < 0.001 else "**" if p_values[i] < 0.01 else "*" if p_values[i] < 0.05 else ""
    print(f"  disturb_{i}: t={t_stats[i]:.2f}, p={p_values[i]:.4f} {sig}")

# ============================================================
# 9. 总结：哪个电场的振打对排放影响最大
# ============================================================
print("\n" + "=" * 60)
print("综合结论")
print("=" * 60)

# 峰值贡献排序
peak_order = sorted([(i, df[f'delta_C_peak_simple_{i}'].median()) 
                     for i in range(1, 5)], key=lambda x: x[1], reverse=True)
print("\n各电场振打瞬时峰值贡献排序 (中位数):")
for i, val in peak_order:
    print(f"  E{i}: {val:.2f} mg/Nm³")

# 残差扰动贡献排序
disturb_order = sorted([(i, beta[i], p_values[i]) for i in range(1, 5)], 
                       key=lambda x: abs(x[1]), reverse=True)
print("\n各电场振打击对浓度残差的解释力排序:")
for i, coef, p in disturb_order:
    sig = "显著" if p < 0.05 else "不显著"
    print(f"  E{i}: β={coef:.6f}, p={p:.4f} ({sig})")

print("\n完成。")