import numpy as np

# 工况0的参数
C_in = 44.41
Q = 460561
U = np.array([45.9, 45.4, 59.2, 58.2])
T = np.array([88.0, 289.0, 60.0, 60.0])

# 第一问常数
alpha = 6.327
K = np.array([79.636, 50.315, 132.613, 166.583])
gamma = np.array([0.00460, 0.00582, 0.00165, 0.00172])
b_coef = -K * gamma

def c_out(U, T, C_in, Q):
    Y = alpha + np.sum(K * U**2 / Q) + np.sum(b_coef * U**2 * T / Q)
    return C_in * 1000 * np.exp(-Y)

def power(U, T):
    return (1301.57 + 1.7915*U[0] + 2.0089*U[1] + 1.8968*U[2] + 2.1145*U[3]
            - 1.0826*T[0] - 1.0781*T[1] - 0.2587*T[2] - 0.2564*T[3]
            + 0.0707*U[0]**2 + 0.0692*U[1]**2 + 0.0646*U[2]**2 + 0.0632*U[3]**2)

# 基准
c0 = c_out(U, T, C_in, Q)
p0 = power(U, T)

# U3 调 1 kV
U_up = U.copy(); U_up[2] += 1.0
dC_dU3 = c_out(U_up, T, C_in, Q) - c0
dP_dU3 = power(U_up, T) - p0

# T1 调 10 s
T_down = T.copy(); T_down[0] -= 10.0
dC_dT1 = c_out(U, T_down, C_in, Q) - c0
dP_dT1 = power(U, T_down) - p0

print(f"U3 +1 kV: ΔC_out = {dC_dU3:.3f} mg/Nm³, ΔP = {dP_dU3:.2f} kW")
print(f"T1 -10 s:  ΔC_out = {dC_dT1:.3f} mg/Nm³, ΔP = {dP_dT1:.2f} kW")
print(f"降浓成本比 (T1/U3) = {abs(dP_dT1/dC_dT1) / abs(dP_dU3/dC_dU3):.2f}")