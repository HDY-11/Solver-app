import pandas as pd
import numpy as np
from scipy.optimize import curve_fit

# ============================================================
# 1. 读取数据并计算 C_out_pred
# ============================================================
df = pd.read_csv(r"C:\Users\24070\Desktop\Cement_ESP_Data.csv")
df = df.dropna(subset=['C_out_mgNm3'])

alpha = 6.327
K = np.array([79.636, 50.315, 132.613, 166.583])
gamma = np.array([0.00460, 0.00582, 0.00165, 0.00172])
b_coef = -K * gamma

def calc_ln_eff(row):
    y = alpha
    for i in range(4):
        y += K[i] * row[f'U{i+1}_kV']**2 / row['Q_Nm3h']
        y += b_coef[i] * row[f'U{i+1}_kV']**2 * row[f'T{i+1}_s'] / row['Q_Nm3h']
    return y

df['C_out_pred'] = df.apply(
    lambda row: row['C_in_gNm3'] * 1000 / np.exp(calc_ln_eff(row)), axis=1
)

# ============================================================
# 2. 筛选饱和区数据：C_out_pred > 50（理论上已超出量程）
# ============================================================
df_sat = df[df['C_out_pred'] > 50].copy()
print(f"全量: {len(df)}, 饱和区 (C_out_pred>50): {len(df_sat)} "
      f"({100*len(df_sat)/len(df):.1f}%)")

x_data = df_sat['C_out_pred'].values
y_data = df_sat['C_out_mgNm3'].values

# ============================================================
# 3. 定义饱和映射函数
# ============================================================
def saturation_model(x, k):
    C_sat = 50.00
    C_min = 48.74
    return C_sat - (C_sat - C_min) * np.exp(-k * x)

# ============================================================
# 4. 拟合
# ============================================================
popt, pcov = curve_fit(saturation_model, x_data, y_data, p0=[0.05], maxfev=5000)
k = popt[0]
k_err = np.sqrt(np.diag(pcov))[0]

y_pred = saturation_model(x_data, k)
ss_res = np.sum((y_data - y_pred)**2)
ss_tot = np.sum((y_data - np.mean(y_data))**2)
R2 = 1 - ss_res / ss_tot
rmse = np.sqrt(np.mean((y_data - y_pred)**2))

# ============================================================
# 5. 线性化检验（加保护，避免 ln(0)）
# ============================================================
eps = 1e-6
C_sat, C_min = 50.00, 48.74
y_protected = np.clip(y_data, C_min + eps, C_sat - eps)
y_linear = np.log((C_sat - C_min) / (C_sat - y_protected))

# 线性回归
mask = np.isfinite(y_linear)
slope, intercept = np.polyfit(x_data[mask], y_linear[mask], 1)
# 线性回归的 R²
y_lin_pred = slope * x_data[mask] + intercept
ss_res_lin = np.sum((y_linear[mask] - y_lin_pred)**2)
ss_tot_lin = np.sum((y_linear[mask] - np.mean(y_linear[mask]))**2)
R2_lin = 1 - ss_res_lin / ss_tot_lin

# ============================================================
# 6. 分类统计：验证压缩映射的存在
# ============================================================
bins = [50, 55, 60, 65, 70, 100]
for i in range(len(bins)-1):
    mask_bin = (x_data >= bins[i]) & (x_data < bins[i+1])
    if mask_bin.sum() > 0:
        print(f"C_out_pred ∈ [{bins[i]},{bins[i+1]}): "
              f"实测均值={y_data[mask_bin].mean():.4f}, "
              f"实测范围=[{y_data[mask_bin].min():.2f},{y_data[mask_bin].max():.2f}], "
              f"样本量={mask_bin.sum()}")

# ============================================================
# 7. 输出报告
# ============================================================
print("\n" + "=" * 70)
print("仪表精度饱和的物理模型验证（仅饱和区 C_out_pred > 50）")
print("=" * 70)
print(f"\n[数据]")
print(f"  全量样本: {len(df)}")
print(f"  饱和区样本: {len(df_sat)} ({100*len(df_sat)/len(df):.1f}%)")
print(f"  C_out_pred 范围: [{x_data.min():.1f}, {x_data.max():.1f}] mg/Nm³")
print(f"  C_out_pred 均值: {x_data.mean():.1f} mg/Nm³")
print(f"  实测值范围: [{y_data.min():.2f}, {y_data.max():.2f}] mg/Nm³")
print(f"  实测值均值: {y_data.mean():.4f} mg/Nm³")
print(f"  实测值标准差: {y_data.std():.4f} mg/Nm³")

print(f"\n[拟合结果]")
print(f"  k = {k:.6f} ± {k_err:.6f}")
print(f"  R² = {R2:.4f}")
print(f"  RMSE = {rmse:.4f} mg/Nm³")

print(f"\n[线性化检验]")
print(f"  ln[(C_sat-C_min)/(C_sat-C_measured)] vs C_out_pred")
print(f"  线性回归 R² = {R2_lin:.4f}")
print(f"  斜率 = {slope:.6f} (理论值 k = {k:.6f}, 偏差 {abs(slope-k)/k*100:.1f}%)")
print(f"  截距 = {intercept:.6f} (理论值 0)")

print(f"\n[结论]")
if R2 > 0.6:
    print(f"  ✓ 指数压缩映射拟合 R²={R2:.3f}，仪表饱和响应得到物理模型证实")
elif R2 > 0.3:
    print(f"  △ R²={R2:.3f}，存在一定解释力，但残差中仍有其他因素")
else:
    print(f"  ✗ R²={R2:.3f}，指数模型不适用——仪表饱和机制可能不是简单的指数型")
print(f"  ✓ 真实浓度从 {x_data.min():.0f} 到 {x_data.max():.0f} mg/Nm³")
print(f"     波动 {x_data.max()-x_data.min():.0f} mg/Nm³")
print(f"     被压缩到 {y_data.max()-y_data.min():.2f} mg/Nm³ 范围内")
print(f"     压缩比 ≈ {(x_data.max()-x_data.min())/(y_data.max()-y_data.min()):.0f}:1")
print("=" * 70)