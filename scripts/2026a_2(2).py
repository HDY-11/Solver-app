import pandas as pd
import numpy as np
from sklearn.cluster import KMeans
from sklearn.preprocessing import StandardScaler
from sklearn.metrics import silhouette_score, calinski_harabasz_score, davies_bouldin_score

# ============================================================
# 读取数据
# ============================================================
df = pd.read_csv(r"C:\Users\24070\Desktop\Cement_ESP_Data.csv")
df = df.dropna(subset=['C_out_mgNm3', 'C_in_gNm3', 'Temp_C'])

X = df[['C_in_gNm3', 'Temp_C']].values
X_scaled = StandardScaler().fit_transform(X)

# ============================================================
# 聚类评估：K=2 到 7
# ============================================================
print("=" * 80)
print("K-means 聚类评估报告")
print("=" * 80)
print(f"聚类变量: C_in (入口粉尘浓度, g/Nm³) + Temp (烟气温度, ℃)")
print(f"样本量: {len(X)}")
print()

results = []
for k in range(2, 8):
    km = KMeans(n_clusters=k, random_state=42, n_init=10)
    labels = km.fit_predict(X_scaled)
    
    sil = silhouette_score(X_scaled, labels)
    ch = calinski_harabasz_score(X_scaled, labels)
    db = davies_bouldin_score(X_scaled, labels)
    
    # 各聚类中心（反标准化）
    centers_scaled = km.cluster_centers_
    centers_raw = StandardScaler().fit(X).inverse_transform(centers_scaled)
    
    # 各聚类样本量
    _, counts = np.unique(labels, return_counts=True)
    
    results.append({
        'K': k,
        '轮廓系数': sil,
        'CH指数': ch,
        'DB指数': db,
        '各聚类样本量': counts,
        '聚类中心_C_in': centers_raw[:, 0],
        '聚类中心_Temp': centers_raw[:, 1]
    })

# 打印汇总表
print(f"{'K':<4} {'轮廓系数':<10} {'CH指数':<12} {'DB指数':<10} {'各聚类样本量'}")
print("-" * 70)
for r in results:
    counts_str = str(r['各聚类样本量'])
    print(f"{r['K']:<4} {r['轮廓系数']:<10.4f} {r['CH指数']:<12.1f} "
          f"{r['DB指数']:<10.4f} {counts_str}")

# 找最优K（轮廓系数最大）
best_k = max(results, key=lambda x: x['轮廓系数'])
print(f"\n推荐K = {best_k['K']} (轮廓系数最大: {best_k['轮廓系数']:.4f})")

# ============================================================
# 最优K的详细结果
# ============================================================
print("\n" + "=" * 80)
print(f"K = {best_k['K']} 各聚类详细描述")
print("=" * 80)

km_final = KMeans(n_clusters=best_k['K'], random_state=42, n_init=10)
df['工况'] = km_final.fit_predict(X_scaled)

for c in range(best_k['K']):
    cluster_data = df[df['工况'] == c]
    print(f"\n--- 工况 {c} (样本量: {len(cluster_data)}) ---")
    print(f"  C_in: 均值={cluster_data['C_in_gNm3'].mean():.1f}, "
          f"中位={cluster_data['C_in_gNm3'].median():.1f}, "
          f"范围=[{cluster_data['C_in_gNm3'].min():.1f}, {cluster_data['C_in_gNm3'].max():.1f}]")
    print(f"  Temp: 均值={cluster_data['Temp_C'].mean():.1f}, "
          f"中位={cluster_data['Temp_C'].median():.1f}, "
          f"范围=[{cluster_data['Temp_C'].min():.1f}, {cluster_data['Temp_C'].max():.1f}]")
    print(f"  Q:    均值={cluster_data['Q_Nm3h'].mean():.0f}, "
          f"中位={cluster_data['Q_Nm3h'].median():.0f}")
    print(f"  C_out: 均值={cluster_data['C_out_mgNm3'].mean():.4f}, "
          f"50.00占比={100*(cluster_data['C_out_mgNm3']==50.0).mean():.1f}%")

# ============================================================
# 物理可解释性：给每个工况命名
# ============================================================
print("\n" + "=" * 80)
print("工况命名建议（基于聚类中心）")
print("=" * 80)

centers = km_final.cluster_centers_
# 反标准化
centers_raw = StandardScaler().fit(X).inverse_transform(centers)

# 计算C_in和Temp的中位数用于判断高低
c_in_median = np.median(centers_raw[:, 0])
temp_median = np.median(centers_raw[:, 1])

for c in range(best_k['K']):
    c_in_level = "高浓度" if centers_raw[c, 0] > c_in_median else "低浓度"
    temp_level = "高温" if centers_raw[c, 1] > temp_median else "低温"
    print(f"  工况 {c}: {c_in_level}·{temp_level} "
          f"(C_in中心={centers_raw[c, 0]:.1f}, Temp中心={centers_raw[c, 1]:.1f})")

print("\n评估完成。")