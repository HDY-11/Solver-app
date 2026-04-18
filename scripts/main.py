import simple_motion;

s = simple_motion.Motion()

s.add_particle(1.0,0.1,0.0,0)
print(s.get_particle(0))

import cupy as cp

# 检查 GPU 可用性
print(f"GPU 可用: {cp.cuda.is_available()}")
print(f"GPU 名称: {cp.cuda.runtime.getDeviceProperties(0)['name'].decode()}")

# 矩阵乘法测试
a = cp.array([[1, 2], [3, 4]])
b = cp.array([[5, 6], [7, 8]])
c = cp.dot(a, b)
print(f"矩阵乘法结果:\n{c}")