# 细线（hairline）放大后变成实宽描边：分支判定、宽度量化与像素亮度

分析对象：`tiny-skia` 0.12.0（本仓库，分支 `task/252`）。
分析目的：解释"放大预览时，一条矢量细线为什么会从 1px 发丝线变成有实际宽度的描边"，
并给出**带方向尺度**的数值例子，让接入方能事先判断：

1. 某个 `(width, transform, anti_alias, blend_mode)` 组合会走哪条绘制路径；
2. 走那条路径后，像素亮度如何随宽度变化；
3. 为什么"把覆盖比例折进画笔透明度"这件事，受抗锯齿开关和叠加方式限制。

文中所有数值都是在本仓库代码上实测得到的（测量方法见 §5），环境：macOS + Rust 1.100，
默认 feature `std,simd,png-format`。所有例子都是**白底 + 不透明黑线 + `SourceOver`**（除非特别注明），
所以"墨量 ink"= 该行/列像素 `(255 - gray) / 255` 之和，单位是"整黑设备像素"。

---

## 0. 结论速览

* 描边分成**两个互斥分支**，由一个只与 `transform` 和 `width` 有关、**与线段方向无关**的判据选择：
  * **hairline 分支**（假粗线）：把宽度折成画刷的透明度，几何上永远只画 1 个设备像素宽的 AA 线。
  * **outline 分支**（真描边）：把路径真正加粗成轮廓多边形，再走普通的 `fill_path`。
* 判据（`treat_as_hairline`，`src/painter.rs:544`）：

  ```
  A = fast_len(T·(1,0))          // 单位用户 x 方向在设备上的长度（八边形范数）
  B = fast_len(T·(0,1))          // 单位用户 y 方向在设备上的长度
  c(w) = w · (A + B) / 2         // 折叠到透明度上的"设备宽度"
  走 hairline  ⇔  A·w ≤ 1 且 B·w ≤ 1   ⇔  w ≤ 1 / max(A,B)
  ```

  其中 `fast_len(x,y) = max(|x|,|y|) + 0.5·min(|x|,|y|)` 是欧氏长度的八边形近似（45° 时高估约 6.07%）。
* 因为是 `(A+B)/2`，判据**方向盲**：`scale(4,1)` 下，横线和竖线在同一个 user 宽度处切换分支，
  可它们的真实设备厚度相差 4 倍。
* 像素亮度规律：
  * hairline：`α_eff = floor(255·floor(256·c(w))/256)`，`gray = 255 − 255·α_paint·α_eff/255`
    （`α_paint` 是画刷自身 alpha）；量化步长 ≈ 1/255，`c(w) < 2/256` 时**完全不可见**。
  * outline + AA：几何覆盖被量化到 **1/4 像素**（`SUPERSAMPLE_SHIFT = 2`）。
  * outline + 非 AA：整数像素，线心落在整数像素中心时 0.99px 会**什么都不画**（见 §2.7）。
* "覆盖折入透明度"只对 **7 种叠加模式**生效（见 §3.2）；其余模式仍按像素覆盖率合成，
  但**丢掉了宽度因子**——0.1px 的线和 0.9px 的线看起来一样粗、一样黑。

---

## 1. 绘制路径

### 1.1 判定函数

`PixmapMut::stroke_path`（`src/painter.rs:330`）先做 dash 展开，然后：

```rust
// src/painter.rs:359
if let Some(coverage) = treat_as_hairline(paint, stroke, transform) {
    ... // hairline 分支
} else {
    let path = path.stroke(stroke, res_scale)...;
    self.fill_path(&path, &paint, FillRule::Winding, transform, mask) // outline 分支
}
```

`treat_as_hairline`（`src/painter.rs:544-584`）的实际规则：

| 条件 | 结果 | 位置 |
| --- | --- | --- |
| `width == 0.0` | `Some(1.0)`（真正的 cosmetic hairline，**不随缩放变粗**） | `:557` |
| `!paint.anti_alias` | `None`（任何 `width > 0` 都走 outline，见 §3.1） | `:561` |
| 平移被清零，只看线性部分 | | `:565-567` |
| 对 `T·(w,0)` 和 `T·(0,w)` 取 `fast_len`，两者都 ≤ 1 | `Some((len0+len1)/2)` | `:570-580` |
| 否则 | `None` | `:583` |

注意三点：

1. **平移不影响判据**，但缩放/旋转/斜切影响。
2. 判据用的向量是**坐标轴方向**，不是"线段法线"，所以同一 transform 下不同走向的线共享同一个阈值。
3. 返回的 `coverage` 就是"折叠系数"，取值恰好 ≈ 设备宽度（单位：设备像素）。

### 1.2 三条实际光栅化路径

| 分支 | 入口 | 关键实现 |
| --- | --- | --- |
| hairline + AA | `scan::hairline_aa::stroke_path`（`src/painter.rs:463`） | `src/scan/hairline_aa.rs`：按斜率逐行/逐列把中线覆盖写到相邻 1~2 个像素的 alpha（u8） |
| hairline + 非 AA | `scan::hairline::stroke_path`（`src/painter.rs:465`） | `src/scan/hairline.rs:109/141`：`blit_h(ix, iy, 1)`，整像素 |
| outline | `fill_path`（`src/painter.rs:445`） | AA：`src/scan/path_aa.rs`（4× 超采样）；非 AA：`src/scan/path.rs`（整数扫描线） |

`width == 0` 在 AA 关闭时仍然走 hairline 非 AA 分支（因为 `width == 0` 的判断在 AA 判断之前），
实测恒为 1px。

### 1.3 折叠系数如何变成像素亮度

hairline 分支里（`src/painter.rs:359-370`）：

```rust
if coverage == 1.0 {
    // 不做任何事
} else if paint.blend_mode.should_pre_scale_coverage() {
    let scale = (coverage * 256.0) as i32;       // 截断
    let new_alpha = (255 * scale) >> 8;          // ≈ floor(255·coverage)
    paint.shader.apply_opacity(new_alpha as f32 / 255.0);
}
```

* `Shader::apply_opacity`（`src/shaders/mod.rs:127`）对 `SolidColor` 是乘 alpha，
  对渐变是乘每个 stop 的 alpha，对 pattern 是乘 `opacity`。
* 之后管线**仍然**会按逐像素几何覆盖率再合成一次：
  * 白名单模式用 `Scale1Float` / `ScaleU8`（`src/pipeline/blitter.rs:91,168`）；
  * 其他模式用 `Lerp1Float` / `LerpU8`（`src/pipeline/blitter.rs:108,185`）。
  * 逐像素覆盖率由 `blit_anti_h` 写进 `ctx.current_coverage`（`src/pipeline/blitter.rs:278`）。

所以对"整数像素中心"上的竖直/水平线，hairline 分支的最终效果是：

```
像素 alpha ≈ α_paint · c(w) · geometry
gray      = 255 − 255 · α_paint · c(w) · geometry      (白底、SourceOver、黑色)
```

`geometry` 由线的亚像素位置决定：线心落在像素中心（如 `x = 10.5`）→ 单列 `geometry = 1`；
线心落在像素边界（如 `x = 10.0`）→ 相邻两列各 `0.5`。

上面两式忽略了 `scale`/`new_alpha` 的两次向下取整；精确形式是
`α_eff = floor(255·floor(256·c(w))/256)`，误差最多 2/255。

`width == 0` 时 `coverage = 1.0`，因为命中 `coverage == 1.0` 的短路，画刷透明度不变，
得到一条标准 1px 线（无论放大多少倍）。

---

## 2. 带方向尺度的数值例子

### 2.1 阈值表：`A`、`B`、`c(w)` 与分支切换点

`A`、`B` 是"每单位用户宽度对应的设备宽度"（八边形范数），`c(w) = w(A+B)/2`，
hairline 成立的条件是 `w ≤ 1/max(A,B)`。

| transform | A | B | c(w) | hairline 的最大 user 宽度 |
| --- | --- | --- | --- | --- |
| 恒等 | 1.0000 | 1.0000 | 1.0000·w | 1.0000 |
| `scale(2,2)` | 2.0000 | 2.0000 | 2.0000·w | 0.5000 |
| `scale(4,4)`（4× 预览） | 4.0000 | 4.0000 | 4.0000·w | **0.2500** |
| `scale(8,8)`（8× 预览） | 8.0000 | 8.0000 | 8.0000·w | 0.1250 |
| `scale(0.5,0.5)`（缩小） | 0.5000 | 0.5000 | 0.5000·w | 2.0000 |
| `rotate(45°)` | 1.0607 | 1.0607 | 1.0607·w | 0.9428 |
| `rotate(30°)` | 1.1160 | 1.1160 | 1.1160·w | 0.8960 |
| `skew(kx=2)`（x' = x + 2y） | 1.0000 | 2.5000 | **1.7500·w** | 0.4000 |
| `scale(4,1)` | 4.0000 | 1.0000 | 2.5000·w | 0.2500 |

读法：在 4× 预览下，`width ≤ 0.25` 的线仍然只是"被调亮的 1px 线"（亮度 = `4w`），
`width = 0.26` 立刻变成几何宽度 0.26·4 = 1.04px 的描边（受 1/4 量化，实测 ink 1.004，见 §2.6）。
而 `skew(kx=2)` 下 0.4 的线被当成 1.75·0.4 = 0.7px 亮度的发丝线，
**即使它的真实法向厚度只有 0.4/√5 ≈ 0.18px**。

### 2.2 固定 user 宽度，逐渐放大（AA 开、`SourceOver`）

线在 user `x = 10`，`width = 0.1`，恒等变换时为 0.1 设备像素：

| scale | 设备宽度 | `c(w)` | 分支 | 实测 ink（整行积分；线心在设备整数坐标，被分成两列） |
| --- | --- | --- | --- | --- |
| 1 | 0.100 | 0.100 | hairline | 0.094 |
| 2 | 0.200 | 0.200 | hairline | 0.196 |
| 4 | 0.400 | 0.400 | hairline | 0.400 |
| 8 | 0.800 | 0.800 | hairline | 0.796 |
| 9.9 | 0.990 | 0.990 | hairline | 0.988 |
| **10.0** | **1.000** | **1.000** | hairline（`coverage == 1.0` 短路） | **1.000** |
| **10.01** | 1.001 | — | **outline** | 1.004 |
| 11 | 1.100 | — | outline | 1.004 |
| 12 | 1.200 | — | outline | 1.004 |

* 放大到 10× 之前，**线的几何宽度一直是 1 个设备像素**，只是"越来越不透明"：
  不透明度 ≈ `scale·w`（本例 `w = 0.1`，所以约为 `0.1·scale`）。
  注意这里的 1px 是"沿一行积分"的总墨量；线心在整数坐标时它被分摊到相邻两列（§2.5），
  单像素 `gray` 是 `(243,243) → (230,230) → (204,204) → (154,153)`，而不是
  `244 → 231 → 192 → 128`（那是线心落在像素中心时的值）。
* 过了阈值后，宽度交给几何：AA 填充把 1.001~1.24 都量化成 1.0（见 §2.6），
  所以 10.01×~12× 之间肉眼反而"不再变粗"。
* `width = 0` 不受此影响：1× / 4× / 16× 实测 ink 恒为 1.000（cosmetic hairline）。

### 2.3 各向异性：判据方向盲，真实厚度不盲

`scale(4,1)`，同一 user `width = 0.25`：

| 线走向 | 真实设备厚度 | `c(w)` | 实测 ink |
| --- | --- | --- | --- |
| 竖直 user 线（法向在 x） | 4·0.25 = **1.00** | 0.625 | 0.624 |
| 水平 user 线（法向在 y） | 1·0.25 = **0.25** | 0.625 | 0.624 |

两条线的渲染结果**完全一样**——因为判据只用 `T·(1,0)`、`T·(0,1)`。后果：

* 横线被"虚胖"成 2.5 倍（0.625 而不是 0.25）；
* 竖线被"缩水"到 0.625 倍（0.625 而不是 1.0）。

再跨过阈值看方向差异（`scale(4,1)`，`width` 只加 0.01；两条线都放在像素边界上：
竖线 device `x = 40`、横线 device `y = 8.0`）：

| 线走向 | `w = 0.25`（hairline，两像素） | `w = 0.26`（outline） | 亮度变化 |
| --- | --- | --- | --- |
| 水平线（真实厚度 0.25 → 0.26） | ink 0.624（176 / 175） | ink 0.498（192 / 191） | **变暗 20%** |
| 竖直线（真实厚度 1.00 → 1.04） | ink 0.624（176 / 175） | ink 1.004（127 / 127） | **变亮 61%** |

同一个 +0.01 的宽度增量，一个方向变暗、另一个方向变亮，是接入方最容易踩的坑。

更极端的情形：`scale(8,1)` 或 `scale(16,1)`（例如只横向拉伸的预览）下，**水平线一过阈值就整条消失**。
阈值处 `c(w)` 仍然把线提亮到 0.55 左右，可跳进 outline 分支后，真实设备厚度只有 0.125px，
被 AA 填充的 1/4 像素量化抹成 0：

| transform | `w = 阈值 − 0.0001` | `w = 阈值` | `w = 阈值 + 0.0001` |
| --- | --- | --- | --- |
| `scale(4,1)`，阈值 0.25 | 列 ink 0.620，`gray = 97` | 0.624，`gray = 96` | **0.502，`gray = 127`**（变淡） |
| `scale(8,1)`，阈值 0.125 | 0.557，`gray = 113` | 0.561，`gray = 112` | **0.000，不可见** |
| `scale(16,1)`，阈值 0.0625 | 0.525，`gray = 121` | 0.529，`gray = 120` | **0.000，不可见** |

这类"横向拉伸"的预览里，一条 1px 左右的水平发丝线在阈值附近会先变亮、再突然消失；
同宽度的竖直线则平滑地继续变粗（`scale(8,1)`：0.561 → 1.004；`scale(16,1)`：0.529 → 1.004），
完全由 §2.3 的方向盲判据造成。

### 2.4 斜切与旋转：八边形范数 + 分支断崖

`skew(kx=2)`（`x' = x + 2y`），user 竖直线。注意它在设备空间里**更接近水平**
（每往下一行，横坐标走 2 像素），所以下表的"行内 ink"是沿一整条设备行对覆盖率做积分，
把这条 2 像素的横向行程也算进去了，大小约为 `2·c(w)`；**不要把它当作厚度**。
真正要看的是最后一列的"分支断崖"：

| user `w` | 计算用的 `c(w)` | 分支 | 行内实测 ink | 说明 |
| --- | --- | --- | --- | --- |
| 0.10 | 0.175 | hairline | 0.341 | 真实法向厚度只有 0.045px，却被提亮到 0.175 |
| 0.20 | 0.350 | hairline | 0.690 | |
| 0.30 | 0.525 | hairline | 1.043 | |
| **0.40** | **0.700** | hairline（临界） | **1.396**（三像素 166/77/166） | 最亮 |
| **0.41** | — | **outline** | **0.502**（两像素 191/191） | **亮度掉到 1/2.8** |
| 0.50 | — | outline | 0.502 | |
| 1.00 | — | outline | 1.004 | |

从 0.40 到 0.41，user 宽度只加了 0.01，行内墨量却从 1.396 掉到 0.502（约 −64%）；中间没有平滑过渡。

`rotate(45°)`，阈值正好是 0.9428（因为八边形范数把 1 单位向量估成 1.0607）：

| `w` | 分支 | 列内实测 ink | 像素 |
| --- | --- | --- | --- |
| 0.93 | hairline | 0.984 | 一个像素 `gray = 4` |
| 0.94 | hairline | 0.996 | `gray = 1` |
| **0.9428** | hairline（临界） | 0.996 | `gray = 1` |
| **0.943** | outline | **1.255** | `207 / 31 / 207` |
| 0.95 | outline | 1.255 | `207 / 31 / 207` |

也就是说：45° 旋转时，0.9 的线在设备空间里法向厚度正好 0.9px，但因为 `fast_len` 高估 6.07%，
折叠系数被算成 1.0607·0.9 = 0.955（单像素 `gray = 12`，列内 ink 0.953）；一旦越过阈值，同一个测量口径
（沿一条竖直列积分）下的墨量从 0.996 跳到 1.255，**中间没有 1.0 附近的取值**。
跳变同时来自两处：分支翻转，以及 AA 填充把 1/4 像素当成最小台阶（§2.6）。

### 2.5 线的亚像素位置：同样的宽度，不同的像素分布

恒等变换、`w = 0.5`：

| 线心位置 | 渲染结果 | 总 ink |
| --- | --- | --- |
| `x = 10.5`（像素中心） | 单列 `gray = 128` | 0.498 |
| `x = 10.0`（像素边界） | 两列 `gray = 191, 191` | 0.502 |
| `x = 10.25` | `223, 159` | 0.502 |
| `x = 10.75` | `160, 223` | 0.498 |

总墨量守恒，但"最暗像素值"写成 width 的函数时，必须固定位置才能复现。
hairline 分支的完整亮度表（`x = 10.5`，`gray = 255 − new_alpha`）：

| `w` | 0.05 | 0.10 | 0.25 | 0.50 | 0.75 | 0.90 | 0.99 | 1.00 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `scale = floor(256w)` | 12 | 25 | 64 | 128 | 192 | 230 | 253 | 256 |
| `new_alpha = (255·scale)>>8` | 11 | 24 | 63 | 127 | 191 | 229 | 252 | 255 |
| 实测 `gray` | 244 | 231 | 192 | 128 | 64 | 26 | 3 | 0 |

线条越靠近阈值，斜率越大（`gray` 从 26 掉到 3，只用了 0.09 的宽度），视觉上像"突然变实"。

### 2.6 outline 分支的台阶

恒等变换、线在像素边界 `x = 10.0`、AA 开。因为填充用 4× 超采样（`SUPERSAMPLE_SHIFT = 2`，
`src/scan/path_aa.rs:21`），横向覆盖率被量化到 **1/4 像素**：

| `w` | 实测 ink | 相关像素 `gray` |
| --- | --- | --- |
| 1.00 | 1.000 | 128 / 127 |
| 1.24 | 1.004 | 127 / 127 |
| 1.26 | 1.506 | 63 / 63 |
| 1.50 | 1.506 | 63 / 63 |
| 1.75 | 1.753 | 63 / 0 |
| 1.88 | 2.000 | 0 / 0 |
| 2.25 | 2.251 | 0 / 0 / 191（三像素） |
| 2.26 | 2.502 | 191 / 0 / 0 / 191（四像素） |

可见 `1.26px`～`1.74px` 的描边**一模一样**（都是 63/63），`1.76px`～`2.24px` 也一模一样（都是 0/0）。
恰好落在 1/4 边界上的宽度（`1.25`、`1.75`）会多出半个像素的灰色（`1.25 → 127/63`、`1.75 → 63/0`），
属于过渡值。这个 1/4 台阶与 hairline 分支的 1/255 量化形成鲜明对比：跨越分支阈值时，
宽度的"分辨率"从 1/255 掉到 1/4。

薄填充（不是描边）同样量化，且可能整体消失（`fill_path`，横带中心对准像素中心 `y = 8.5`）：

| 高度 | 0.1 | 0.2 | 0.3 | 0.5 | 0.8 | 1.0 |
| --- | --- | --- | --- | --- | --- | --- |
| 实测 | 不可见 | 不可见 | `gray = 127`（0.5px） | 0.5px | 1.0px | 1.0px |

### 2.7 `anti_alias = false`

AA 关闭时，`treat_as_hairline` 对任何 `width > 0` 都返回 `None`（`src/painter.rs:561`），
所以**没有折叠机制**，宽度是纯几何的，随后被非 AA 扫描线按整像素取整
（`src/scan/path.rs`：`blit_h(fdot16::round_to_i32(x), y, width)`）：

| 线心 `x` | `w = 0.1` | 0.5 | 0.9 | 0.99 | 1.0 | 1.01 | 1.5 | 2.0 | 2.5 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 10.00 | 0px | 0px | 0px | 0px | **1px** | 2px | 2px | 2px | 2px |
| 10.25 | 0px | 1px | 1px | 1px | 1px | 1px | 1px | 2px | 3px |
| 10.50 | 1px | 1px | 1px | 1px | 1px | 1px | 1px | 2px | 3px |
| 10.75 | 0px | 0px | 1px | 1px | 1px | 1px | 2px | 2px | 2px |

注意 `w = 0` 那一列（未列出）在 AA 关闭时仍然画 1px；而 `0 < w < 1` 的线可能**完全消失**。
4× 预览下同样如此：`width = 0.24`（设备 0.96px）→ 0px；`0.25`（1.00px）→ 1px；`0.26`（1.04px）→ 2px。

### 2.8 折叠的量化下限

`scale = (c·256) as i32` 会向下取整，`new_alpha = (255·scale) >> 8` 再压一次。
`w ≤ 1/256` 时 `scale = 0`；`1/256 < w < 2/256` 时 `scale = 1`、`new_alpha = 0`。实测（`x = 10.5`）：

| `w` | 0.0050 | 0.0070 | 0.0078 | 0.0079 | 0.0080 | 0.0100 | 0.0200 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `gray` | 255 | 255 | 255 | 254 | 254 | 254 | 251 |

即：**设备宽度小于 2/256 ≈ 0.0078px 的线在 hairline 分支里是彻底不可见的**。
（如果同一宽度改写走 outline 分支，AA 填充的 1/4 量化也会把它抹平，见 §2.6。）

---

## 3. 抗锯齿与叠加方式如何限制"把覆盖折入画笔透明度"

这一节解释代码为什么会这样，以及接入方能改什么、不能改什么。

### 3.1 抗锯齿开关——直接取消折叠机制

* `anti_alias = true`：才有 hairline 分支，才有 `coverage → shader opacity` 的折叠。
* `anti_alias = false`：`treat_as_hairline` 直接 `None`（`src/painter.rs:561-563`），
  任何 `width > 0` 都做真描边 + 非 AA 填充（`src/scan/path.rs`）。
  * 没有折叠，所以宽度不会"变成亮度"；
  * 但覆盖面被整像素取整，细线可能消失；
  * `width = 0` 例外：仍然是 1px（`width == 0` 的判断在前，走非 AA hairline）。
* 结论：想用"亮度代表宽度"这个技巧，必须开 AA；想保证"宽度就是几何宽度"，反而应该关 AA 或绕过描边。

### 3.2 叠加方式——只有 7 种模式接受折叠

`BlendMode::should_pre_scale_coverage`（`src/blend_mode.rs:68-91`）是一张白名单：

```rust
matches!(self,
    Destination | DestinationOver | Plus | DestinationOut |
    SourceAtop | SourceOver | Xor)
```

`src/painter.rs:363` 只有这张表里的模式才会执行 `apply_opacity`。实测（hairline，`w` 从 0.1 到 0.9，
`x = 10.5`，底 `gray = 100`，画刷 `gray = 200`，取 `x = 10` 的像素值）：

| 模式 | `w = 0.1` | `w = 0.5` | `w = 0.9` | 宽度折叠是否生效 |
| --- | --- | --- | --- | --- |
| `SourceOver` | 110 | 150 | 191 | ✅ |
| `SourceAtop` | 110 | 150 | 190 | ✅ |
| `Xor` | 91 | 50 | 11 | ✅ |
| `DestinationOut` | 91 | 50 | 11 | ✅ |
| `Plus` | 119 | 200 | 255 | ✅ |
| `DestinationOver` | 100 | 100 | 100 | ✅（但公式与 dst 有关，见注） |
| `Destination` | 100 | 100 | 100 | ✅（无变化，本来就是 no-op） |
| `Source` | 200 | 200 | 200 | ❌ |
| `SourceIn` | 200 | 200 | 200 | ❌ |
| `DestinationIn` | 100 | 100 | 100 | ❌ |
| `Multiply` | 79 | 79 | 79 | ❌ |
| `Screen` | 221 | 221 | 221 | ❌ |
| `Darken` | 100 | 100 | 100 | ❌ |
| `Lighten` | 200 | 200 | 200 | ❌ |
| `Overlay` | 157 | 157 | 157 | ❌ |
| `Modulate` | 79 | 79 | 79 | ❌ |

（注：`DestinationOver` / `Destination` 在白名单里，但它们的公式不使用源 alpha，
白底不透明底图上自然看不出变化。）

**为什么不能对所有模式折叠**：把 coverage 乘进 RGB 和 alpha，等于把"覆盖率"和"源 alpha"
合并成一个数；而 `Multiply`、`Screen`、`Darken`、`Overlay` 等模式在公式里需要**原始 source-alpha**
（例如 `s·inv(da) + d·inv(sa) + s·d`）。预先缩放会破坏这个项，结果就不是"覆盖率"而是"更透明的颜色"。
代码注释（`src/blend_mode.rs:69-81`）写得很清楚：这是 Skia `SkBlendMode_SupportsCoverageAsAlpha()`
的细分版本。

**但要区分两件事**：逐像素 AA 覆盖率本身对**所有**模式都有效（走 `Lerp1Float` / `LerpU8`，
`src/pipeline/blitter.rs:108,185`）。实测 `Multiply`（底 `gray = 100`、画刷 `gray = 200`）在
像素边界上的水平线上，两个像素各 50% 覆盖，得到 `y7 = 90, y8 = 90`——介于纯乘法的 79
与未覆盖的背景 100 之间；`fill_path` 的 0.25px 薄带用 `Multiply` 也会得到 `gray = 95`。
非白名单模式丢掉的只是**宽度因子**，不是"抗锯齿"。

对只关心可见性的接入方，这意味着：

> 如果在 4× 预览里用 `Multiply` / `Screen` / `Darken` 等模式画 0.25 user 宽的线，
> 它会和 0.01 宽的线看起来一模一样（都是 1px 满强度），因为宽度根本没被编码进任何地方。

### 3.3 两套 coverage 机制

| 机制 | 位置 | 作用对象 | 谁受影响 |
| --- | --- | --- | --- |
| 宽度折叠（paint opacity） | `src/painter.rs:361-370` | shader 的 alpha（整体） | 只有白名单 7 种模式 |
| 逐像素覆盖率 | `Scale1Float`/`ScaleU8`（白名单）或 `Lerp1Float`/`LerpU8`（其他） | 每个像素 | 所有模式 |

`Scale*` 与 `Lerp*` 的差别在于"先乘再加"还是"在 dst/src 之间线性插值"：
前者能保持预乘语义，后者在源已有 alpha 时等于"按覆盖率做 alpha 混合"。
两者都会读取 `ctx.current_coverage`（`src/pipeline/highp.rs:385,395`、`src/pipeline/lowp.rs:444,454`），
只是在 `blit_anti_h` 一条一条 run 地设置（`src/pipeline/blitter.rs:278`）。

### 3.4 其他会干扰折叠的因素

* **Mask**：`stroke_path(..., mask)` 会在 shader 之后再乘一次 mask（`MaskU8` 阶段），
  折叠系数与 mask 相乘，效果叠加。
* **渐变 / pattern**：`apply_opacity` 分别乘每个 stop / pattern opacity，语义一致；
  但 pattern 还带自己的 `PixmapPaint.opacity`，两者相乘。
* **颜色空间**（`paint.colorspace`）：不影响折叠系数本身，只影响 expand/compress 后的数值
  （默认 `Linear` 不做 gamma）。
* **`force_hq_pipeline`**：不改变折叠逻辑，只改精度。实测 `Multiply` 在低精度 `u16` 下是 79，
  HQ 下是 78，`Screen` 是 221 / 222。对外只需知道"±1 的舍入差异"。
* **DrawTiler**（`src/painter.rs:372`，`MAX_DIMENSIONS = 8191`，任一边更大时启用）：
  折叠只在进入 tile 循环之前做一次（`:361-370`），之后每个 tile 克隆同一个已折叠的 paint，
  所以各 tile 系数一致，不会出现接缝。
* **`coverage == 1.0` 的精确比较**（`src/painter.rs:361`）：只有当 `(len0+len1)/2` 恰好等于 1.0 时
  才短路（例如恒等变换下 `w = 1.0`）。它和"折叠 1.0"等价，跳过只是为了省一次乘法。
* **`apply_opacity` 的 clamp**（`src/color.rs:326-329`）：`new_alpha ≤ 255` 恒成立（因为两条 `len ≤ 1`），
  所以实际不会触发 clamp；但如果接入方自己在 shader 上设了不透明 alpha，折叠仍能正常工作。

### 3.5 一句话总结限制

> 折叠 = `paint_alpha × c(w)`，它只能作用在"颜色透明度"这一个自由度上；
> 所以它要么被 AA 开关整个关掉（`anti_alias = false`），
> 要么被叠加模式白名单筛掉（7 种之外不折叠），
> 要么被 1/255 的量化吃掉（`c < 2/256` 不可见）。

---

## 4. 接入方判定清单

给定 `(width, transform, anti_alias, blend_mode)`：

```text
1. width == 0                       → cosmetic hairline：永远 1 设备像素，不随缩放变粗，
                                       总墨量 = paint_alpha（亚像素位置只影响它在相邻像素间的分配）
2. !anti_alias && width > 0         → outline(non-AA)：整像素台阶，0.99px 可能是 0，
                                       无折叠
3. anti_alias && width > 0:
   A = oct(T·(1,0)); B = oct(T·(0,1))
   if width·A ≤ 1 && width·B ≤ 1:
        → hairline：几何 1px，亮度 ≈ paint_alpha · width·(A+B)/2
                    （仅 blend_mode ∈ {Destination, DestinationOver, Plus,
                      DestinationOut, SourceAtop, SourceOver, Xor}）
        阈值 width* = 1 / max(A,B)
   else:
        → outline(AA)：几何宽度 = 真实设备厚度，覆盖率量化到 1/4 px
```

实用结论：

* **预览缩放时想得到"真的变粗"的线**：不要把 `width` 压到阈值以下；或者干脆自己
  `path.stroke(...)` 成轮廓再 `fill_path`，绕过 `treat_as_hairline`。
* **想保留"细线靠亮度表达"的美感**：保持 `anti_alias = true`、`width ≤ 1/max(A,B)`、
  叠加模式取白名单内的值；注意 `width < 0.008` 会直接消失。
* **各向异性缩放/斜切下**：判据用的是两条坐标轴方向的平均值，不是线段法线；
  横线和竖线会在同一 `width` 处切换；切换处的亮度跳变幅度可达 20%（`scale(4,1)` 横线）
  到接近 3 倍（`skew(kx=2)`），甚至直接消失（§2.3、§2.4）。
* **要保证跨模式一致**：把细线先画到一个透明图层（内部用 `SourceOver`，让折叠生效），
  再用目标 `blend_mode` 把整个图层合成上去——这样宽度信息已经烧进像素 alpha，
  后续合成就只剩"逐像素覆盖率"这一件事，不受白名单限制。

---

## 5. 复现方法

所有表格都来自同一个最小 harness（放在 `target/` 下的临时 crate，依赖本仓库 path 依赖）：

```rust
use tiny_skia::*;

fn draw(w: f32, ts: Transform, aa: bool, bm: BlendMode, at: f32) -> Pixmap {
    let mut p = Pixmap::new(200, 40).unwrap();
    p.fill(Color::WHITE);
    let mut pb = PathBuilder::new();
    pb.move_to(at, -40.0);
    pb.line_to(at, 120.0);            // 竖直线
    let path = pb.finish().unwrap();

    let mut paint = Paint::default();
    paint.set_color_rgba8(0, 0, 0, 255);
    paint.anti_alias = aa;
    paint.blend_mode = bm;

    let mut stroke = Stroke::default();
    stroke.width = w;
    p.stroke_path(&path, &paint, &stroke, ts, None);
    p
}

// ink = 该行所有像素 (255 - gray) / 255 之和，单位"整黑设备像素"
fn ink(p: &Pixmap, y: u32) -> f32 {
    (0..p.width())
        .map(|x| 255 - u32::from(p.pixel(x, y).unwrap().red()))
        .sum::<u32>() as f32
        / 255.0
}
```

`Cargo.toml`：

```toml
[package]
name = "hairline_probe"
version = "0.1.0"
edition = "2021"

[dependencies]
tiny-skia = { path = "../.." }

[workspace]
```

同样的 harness 也可以把 `move_to/line_to` 换成水平线，或用 `fill_path` + 矩形来测 §2.6 的填充量化。

---

## 6. 代码索引

| 主题 | 位置 |
| --- | --- |
| 描边分支入口 | `src/painter.rs:330`（`stroke_path`）、`:359`（折叠分派）、`:445`（outline 回退） |
| 折叠系数计算 | `src/painter.rs:544-584`（`treat_as_hairline`，`fast_len` 在 `:545-553`） |
| 折叠执行 | `src/painter.rs:361-370`（`coverage == 1.0` / `scale` / `new_alpha` / `apply_opacity`） |
| hairline 光栅化（AA） | `src/scan/hairline_aa.rs:226`、`:321`（`do_anti_hairline`）、`:617-790`（四种 `AntiHairBlitter`） |
| hairline 光栅化（非 AA） | `src/scan/hairline.rs:30`、`:39`、`:109`、`:141` |
| 逐像素覆盖率注入 | `src/pipeline/blitter.rs:263-296`（`blit_anti_h`）、`:298-308`（`blit_v`）、`:310-332`（`blit_anti_h2`/`blit_anti_v2`） |
| 管线阶段选择 | `src/pipeline/blitter.rs:89-115`（`Scale1Float`/`Lerp1Float`）、`:166-192`（`ScaleU8`/`LerpU8`） |
| 阶段实现 | `src/pipeline/highp.rs:357`（`scale_u8`）、`:385`（`scale_1_float`）、`:395`（`lerp_1_float`）；`src/pipeline/lowp.rs:384`、`:444`、`:454` |
| 白名单 | `src/blend_mode.rs:68-92`（`should_pre_scale_coverage`） |
| 透明度折叠语义 | `src/shaders/mod.rs:127-145`、`src/color.rs:326-329` |
| AA 填充的超采样（1/4 px） | `src/scan/path_aa.rs:21-25`、`:284-287`（`coverage_to_partial_alpha`） |
| 非 AA 填充（整像素） | `src/scan/path.rs:199-310`（`walk_edges`） |
| outline 的容差缩放 | `path/src/stroker.rs:280-291`（`compute_resolution_scale`） |
