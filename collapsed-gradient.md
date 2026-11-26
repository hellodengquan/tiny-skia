# 线性渐变在小尺寸下退化成实色时，填充色到底是什么

面向对象：美术工具（图标/SVG 导出器）维护者。
代码基线：本仓库 tiny-skia 0.12.0。文中所有数值都在本仓库实测过，并给出可复算步骤。

---

## 0. 结论速览

线性渐变在**构造时**就可能被"压扁"成一个 `Shader::SolidColor`（`src/shaders/linear_gradient.rs:55-80`）。
一旦压扁，渲染器就只会画一种颜色，而且**再也回不去**：之后的绘制变换、分块（DrawTiler）平移都不会影响它（`src/shaders/mod.rs:96-112`，`SolidColor` 分支是空实现）。

| 铺色模式 | 退化后的填充色 | 一句话 |
| --- | --- | --- |
| `Pad`（延展） | **最后一个色标的颜色本身**（保留其 alpha） | `linear_gradient.rs:67-72` |
| `Repeat` / `Reflect`（周期铺色） | **按区间长度加权的"未预乘"颜色平均**（alpha 一并平均） | `linear_gradient.rs:73-78`、`112-162` |

两条容易踩的事实：

1. 判定用的是**渐变自身的局部长度** `(end - start).length()`（f32），阈值 `1/32768 = 3.0517578125e-5`（`src/shaders/gradient.rs:18`，`linear_gradient.rs:50-55`）。
   **绘制变换（`fill_path`/`fill_rect` 的 `transform`）不参与判定**。
2. 平均是在**未预乘**的颜色上做的，所以"完全透明色标里藏着的 RGB"依然参与平均，之后整体再乘平均 alpha —— 结果与"逐色标预乘后平均"不等价（见 §2.2 尾注）。

---

## 1. 判定点

```rust
// src/shaders/linear_gradient.rs:42-88（节选）
if stops.is_empty() { return None; }
if stops.len() == 1 { return Some(Shader::SolidColor(stops[0].color)); }

let length = (end - start).length();      // 局部坐标，f32
if !length.is_finite() { return None; }

if length.is_nearly_zero_within_tolerance(DEGENERATE_THRESHOLD) {   // length <= 3.0517578125e-5
    match mode {
        SpreadMode::Pad => return Some(Shader::SolidColor(stops.last().unwrap().color)),
        SpreadMode::Reflect | SpreadMode::Repeat => {
            return Some(Shader::SolidColor(average_gradient_color(&stops)));
        }
    }
}

transform.invert()?;                      // 注意：在退化分支之后才检查
let unit_ts = points_to_unit_ts(start, end)?;
```

要点：

- `is_nearly_zero_within_tolerance(t)` 的定义是 `|self| <= t`（`path/src/scalar.rs:68-71`），所以**恰好等于**阈值也走退化分支。
- `length` 是 `sqrt(dx*dx + dy*dy)`（`path/src/lib.rs:150-158`）。它是"色标坐标系里两点之间的距离"，与画布、像素、绘制变换都无关。
- 单色标走另一条短路：直接返回该色标（`linear_gradient.rs:46-48`）；空色标返回 `None`。
- 退化分支发生在 `Gradient::new` **之前**，用的是**作者原样传入的 `stops`**，不涉及后来在 `Gradient::new` 里补的 0/1 哨兵色标（`src/shaders/gradient.rs:59-74`）。所以 `Pad` 取的是"艺术家写的最后一个色标"，而不是补出来的 1.0 哨兵。
- `start == end` 时 `length == 0`，先命中退化分支，**不会**返回 `None`——尽管文档注释（`linear_gradient.rs:29-33`）写的是 "Returns `None` when … `start` == `end`"。文档是旧的，行为以代码为准（实测：`start==end` + `Pad` → 末色标，`Repeat` → 平均色）。

---

## 2. `Repeat` / `Reflect`：可复算的加权平均

### 2.1 权重公式

`average_gradient_color`（`linear_gradient.rs:112-162`）对 `t ∈ [0,1]` 上的**分段线性颜色**做积分再除以区间长度 1。等价闭式（要求色标位置单调不减；实践中美术工具都应按升序导出）：

设色标为 `(p_0,C_0) … (p_n,C_n)`，`p_i ∈ [0,1]`（`GradientStop::new` 已把位置 clamp 到 `[0,1]`，NaN 视为 0，见 `src/shaders/gradient.rs:32-37` 与 strict-num 的 `new_clamped`），则

```
w_0 = (p_0 + p_1) / 2
w_i = (p_{i+1} - p_{i-1}) / 2      (0 < i < n)
w_n = (2 - p_{n-1} - p_n) / 2
```

`Σ w_i = 1`（可自检）。直觉：每个色标的权重 = 它"左右各半段"的长度；首尾色标还要吃掉 `[0,p_0]` 和 `[p_n,1]` 这两段延展（因为零点周期下，无限次重复等价于把端色铺满两端）。

```text
位置:  0        p0        p1              p2        1
       |---------|---------|---------------|---------|
颜色:       C0        C1             C2
权重:  |--p0--|--(p1-p0)/2--|--(p2-p1)/2--|--(1-p2)--|
         C0          C1            C2
       C0 权重 = p0 + (p1-p0)/2         （左半段 + 左侧延展区）
       C1 权重 = (p1-p0)/2 + (p2-p1)/2 = (p2-p0)/2
       C2 权重 = (p2-p1)/2 + (1-p2)    （右半段 + 右侧延展区）
```

### 2.2 本文示例（间距不等 + 带透明度）

```
C0 = (200,  40,  40, 255)  @ p0 = 0.10
C1 = ( 40, 200,  40, 128)  @ p1 = 0.50
C2 = ( 40,  40, 200,  64)  @ p2 = 0.85
```

```
w0 = (0.10 + 0.50)/2            = 0.300
w1 = (0.85 - 0.10)/2            = 0.375
w2 = (1 - 0.85) + (0.85-0.50)/2 = 0.150 + 0.175 = 0.325
                                       Σ = 1.000
```

通道（0..255 空间）：

```
R = 0.300*200 + 0.375* 40 + 0.325* 40 =  88.0
G = 0.300* 40 + 0.375*200 + 0.325* 40 = 100.0
B = 0.300* 40 + 0.375* 40 + 0.325*200 =  92.0
A = 0.300*255 + 0.375*128 + 0.325* 64 = 145.3
```

所以 `average_gradient_color` 返回 `Color::from_rgba(88/255, 100/255, 92/255, 145.3/255)`，
`to_color_u8()` 显示为 `(88, 100, 92, 145)`（`Color::to_color_u8`，`src/color.rs:357-360`）。

注意：这是**未预乘**的值。`SolidColor` 渲染路径会把它展开色彩空间、预乘、再量化为 u16（`src/shaders/mod.rs:83-86` → `RasterPipelineBuilder::push_uniform_color`，`src/pipeline/mod.rs:380-396`）：

```
α          = 145.3 / 255 = 0.569804
premul 归一 = (88/255·α, 100/255·α, 92/255·α, α) = (0.196635, 0.223453, 0.205566, 0.569804)
push_uniform_color 里 rgba[i] = (premul[i] * 255 + 0.5) as u16   // f32 -> u16，截断
   R: 0.196635*255 + 0.5 =  50.64 -> 50
   G: 0.223453*255 + 0.5 =  57.48 -> 57
   B: 0.205566*255 + 0.5 =  52.92 -> 52
   A: 0.569804*255 + 0.5 = 145.80 -> 145
u16 源色 = (50, 57, 52, 145)
```

> 对比：若改成"先逐色标预乘、再按同样权重平均"，RGB 的预乘值会是 `(70.79, 52.91, 35.84)`，反预乘后约 `(124, 93, 63)`，明显比 `(88, 100, 92)` 偏暖偏亮。
> 两者不等，说明**透明端色标藏在 RGB 里的颜色会被计入平均**，这是 `average_gradient_color` 的既有语义（alpha 仍然一并平均，所以不会"凭空变亮"到不可接受，但色相会跑）。
> 极端例子：黑(a=255) → 白(a=0) 两色标，退化 `Repeat` 得到 `(128,128,128,128)`，等效于"50% 灰"而不是"黑或白"。

### 2.3 周期铺色的像素贡献（可逐位复算）

渲染时 `SourceOver` 的低精度合成（`src/pipeline/lowp.rs:534-541`）为

```
out = src + div255(dst * (255 - src_a)),   div255(v) = (v + 255) >> 8   // lowp.rs:849-854
```

以 `src = (50, 57, 52, 145)`（`255 - 145 = 110`）：

| 背景 | 计算 | 最终像素（输出 PNG 为未预乘） |
| --- | --- | --- |
| 全透明 | 不合成，直接存储 `(50,57,52,145)` | 预乘缓冲 `(50,57,52,145)`；PNG `(88,100,91,145)`¹ |
| 白 255 | `div255(255*110) = 110` → `50+110=160, 57+110=167, 52+110=162` | `(160,167,162,255)` |
| 黑 0 | `div255(0)=0` | `(50,57,52,255)` |
| 中灰 128 | `div255(128*110) = (14080+255)>>8 = 55` → `(105,112,107)` | `(105,112,107,255)` |

¹ PNG 写出前会 `demultiply`（`src/color.rs:160-173`）：`52 / (145/255) = 91.4 → 91`，所以和理论上的 92 差 1；这是 8 位往返的舍入，不是算法错误。

---

## 3. `Pad`：可复算的像素贡献

`Pad` 直接把**末色标**当实色（`linear_gradient.rs:67-72`），连平均都不算。示例里即

```
C2 = (40, 40, 200, 64)          // 未预乘
α  = 64/255 = 0.250980
premul = (40*α, 40*α, 200*α) = (10.039, 10.039, 50.196)
u16 源色 = (10, 10, 50, 64)
```

`255 - 64 = 191`，同样用 `out = src + div255(dst*191)`：

| 背景 | 计算 | 最终像素 |
| --- | --- | --- |
| 全透明 | 直接存储 | 预乘 `(10,10,50,64)`；PNG `(40,40,199,64)` |
| 白 255 | `div255(255*191)=191` | `(201,201,241,255)` |
| 黑 0 | `div255(0)=0` | `(10,10,50,255)` |
| 中灰 128 | `div255(128*191)=(24448+255)>>8=96` | `(106,106,146,255)` |

高精度管线（`Paint::force_hq_pipeline` 或 `colorspace != Linear` 触发，`src/pipeline/mod.rs:409-450`）用 f32 合成再 `round(255)`（`src/pipeline/highp.rs:669-679`、`1342-1344`）。上述几组实测与 lowp 一致；一般情形两条管线可能相差 ±1，不要用它来对齐像素级夹具。

**关键差异**：`Pad` 的退化色是"末色标"，不是"平均色"。示例中 `(40,40,200,64)` 和周期模式的 `(88,100,92,145)` 是两种完全不同的颜色。

---

## 4. 什么会让渐变"进入"退化分支

### 4.1 改变局部渐变长度：会，而且是断崖式

判定只读 `start` / `end`。让两者局部距离从阈值上下穿过，行为在那个点突变。示例（同一组色标，`Pad`，把色标的 x 从 1.0 开始、图标占 x∈[0,1]，即图标落在渐变左端外侧）：

| 局部长度 | 分支 | 图标实际像素（白底） |
| --- | --- | --- |
| `3.11e-5`（略大于阈值） | 正常渐变 | `(200,40,40)` — 就是 C0 |
| `3.0517578e-5`（恰好等于） | 退化 `Pad` | `(201,201,241)` — C2 合成到白底 |
| `1.53e-5`（小于） | 退化 `Pad` | `(201,201,241)` |

同一个"几乎为零"的渐变，跨过阈值前后颜色从 C0 跳到 C2，因为阈值以上其实还在画真正的渐变，只是这段渲染台上的 `t` 全部落在 `t<=0` 一侧。

再比较**图标横跨**渐变两端时（色标从 x=3.5 到 3.5+3.11e-5，画 8 px，**黑底**，便于直接读出预乘后的源色）：

```
Pad 略高于阈值: (200,40,40) ×4   (10,10,50) ×4   ← 1px 级的硬边两色切分
Pad 低于阈值  : (10,10,50) ×8                     ← 整块单一颜色（= C2 的预乘值）
```

同理，`Repeat` 略高于阈值时会出现周期性色带（黑底实测同一像素行：`(24,101,21) (65,98,28) (118,80,33) (189,46,40) (200,40,40) (10,10,50) …`），而低于阈值是平铺的均匀色 —— 黑底 `(50,57,52)`、白底 `(160,167,162)`。**阈值两侧不是"渐变→同色"，而是"渐变→另一种观感"。**

### 4.2 阈值本身不是标称长度，而是 f32 可表示长度

`(end - start)` 是 f32 减法：`end = start + nominal` 这一步会先把 `end` 舍入到 `start` 量级上的浮点格点（ULP），再做减法。所以"实测长度"是 `round(nominal / ULP) * ULP`，可能变成 0，也可能翻倍。

以标称间距 `3.5e-5`、`Pad` 为例：

| 起点 x | 该量级 ULP | `nominal/ULP` | 实测 `(end-start).length()` | 是否退化（≤ 3.0517578e-5） |
| --- | --- | --- | --- | --- |
| 64 | 7.63e-6 | 4.59 → 5 | `3.81e-5` | 否 |
| 128 | 1.526e-5 | 2.29 → 2 | `3.05e-5` | **是** |
| 256 | 3.052e-5 | 1.15 → 1 | `3.05e-5` | **是** |
| 512 | 6.104e-5 | 0.57 → 1 | `6.10e-5` | 否 |
| 1024 | 1.221e-4 | 0.29 → 0 | `0` | **是** |
| 4096 | 4.883e-4 | 0.072 → 0 | `0` | **是** |

注意 256 与 512 的结论相反——这就是量化抖动。另外，能被表示出来的最小正间距约为**半个 ULP**（x=128 时 ≈ `7.6e-6`，x=1024 时 ≈ `6.1e-5`）：低于这个量级的标称间距一律变成 0。
**导出器如果把渐变的绝对坐标留在几千单位的画板上，同时间距极小，很容易被误判为退化；反过来，阈值附近的结果还会随坐标平移而跳变。**

### 4.3 改变绘制变换：不会进入退化分支

绘制变换（`Pixmap::fill_path(..., transform, ...)`）在 `src/painter.rs:302-313` 里做的是：

```rust
let mut paint = paint.clone();
paint.shader.transform(transform);      // 只把矩阵 post_concat 到 base.transform
self.fill_path(&path, &paint, fill_rule, Transform::identity(), mask)
```

而退化判定发生在更早的 `LinearGradient::new`，只看 `start`/`end`。所以：

1. **把图标缩小 k 倍（在绘制变换里）不会触发退化。** 你会得到一条被压在亚像素 `t` 区间里的真渐变：每个像素取到的是很小一段色带（看起来"接近实色"，但并非单一颜色），并且边缘覆盖率由 AA 决定。实测（同一组色标、局部长度 100、绘制缩放依次 1 / 0.05 / 0.02 / 0.01 / 0.005）`Shader` 始终不是 `SolidColor`，首个像素依次是 `(200,40,40)` → `(200,40,40)` → `(162,129,81)` → `(147,227,147)` → `(201,201,241)`；最后一档已经是"形状不足 1px"的覆盖率效应，而不是渐变塌缩。
2. **把图标放大回去也救不回已塌缩的渐变。** 实测：局部长度 `1e-6` 的退化渐变，按 1 / 1e3 / 1e6 三种绘制缩放渲染，输出仍是同一平色（`Pad` 恒为末色标，`Repeat` 恒为平均色）。
3. **构造时传入的 `transform` 也不影响判定。** 判定在 `transform.invert()?`（`linear_gradient.rs:82`）之前，且 `points_to_unit` 只由 `start`/`end` 生成（`linear_gradient.rs:99-110`）。实测：正常长度 + 构造变换缩放 `1e-6` → 仍是渐变；退化长度 + 任何构造变换（无论是 rank-1 还是 `scale(0,0)`）→ 仍是 `SolidColor`，因为根本走不到第 82 行的可逆性检查。
4. 另有一个边角行为：绘制变换把 `base.transform` 变成奇异矩阵时，`Gradient::push_stages` 里 `self.transform.invert()` 失败，整个管线构建返回 `false`，`RasterPipelineBlitter::new` 随之返回 `None`（`src/shaders/gradient.rs:115-121`、`src/pipeline/blitter.rs:82-84`），这次绘制被**悄悄跳过**——既不报错，也不回退成实色。实测 rank-1 矩阵 `(sx,ky,kx,sy,tx,ty)=(0,0,1,0,0,0)` 时背景像素原样保留（`invert()` 返回 `None`，见 `path/src/transform.rs:325-334`）。
   而 `scale(0,0)`（以及 `Transform::from_row(0,0,0,0,0,0)`）走的是另一条路：`invert` 在 `is_scale_translate` 分支里算 `1/0 = inf`（`path/src/transform.rs:309-324`），`push_stages` 只检查 `invert()` 是否 `Some`（`gradient.rs:115`），于是 `ts = ts.post_concat(points_to_unit)` 含 inf/NaN；`push_transform` 因为 `ts.is_finite()` 为假而**不压入 Transform 阶段**（`src/pipeline/mod.rs:373-378`），渐变直接拿屏幕像素坐标当 `t` 用。
   实测（红→蓝两色标、10px 宽矩形）：种子阶段给的是 `t = x + 0.5`（`src/pipeline/highp.rs:256-270`）。`Pad` 下首像素 `t=0.5` 得中色 `(128,0,128)`，之后 `t≥1` 全被 clamp 成末色 `(0,0,255)`；`Repeat` 下 `frac(0.5)=0.5`、`frac(1.5)=0.5`……整行恒为中色 `(128,0,128)`。这类"变换不可逆但没报错"的观感都属于未定义行为，工具侧应直接禁止奇异矩阵。

一句话给工具作者：**"小"有两种，只有一种会退化。** 把缩放烘进渐变的 `start`/`end` 会触发；把缩放留在绘制变换里不会。

---

## 5. 其他会改变最终填充色的细节

1. **色彩空间**：`average_gradient_color` 只在色标的**存储值**上做线性平均，没有做 `expand`。之后 `SolidColor` 分支才 `cs.expand_color(...)`（`src/shaders/mod.rs:84`），落盘前再用 `compress_stage`（`src/pipeline/blitter.rs:112-115`）。这带来两点：

   - **纯色不透明时** expand/compress 互为逆运算，所以退化色与 `Linear` 完全一致（实测 `(88,100,92,255)` 三种 colorspace 落盘都是 `(88,100,92)`）。但**半透明时不抵消**：预乘发生在 expand 之后、alpha 不参与 expand，`compress` 不能还原（实测同色 `a=145`：`Linear (50,57,52)`、`Gamma2 (66,75,69)`、`SimpleSRGB (68,77,71)`）。
   - 正常渐变是"**先把每个色标展开到线性空间，再插值**"（`src/shaders/gradient.rs:149-160`、`199-231`），与"先在存储空间平均、再展开"并不等价：黑→白两色标时，`Gamma2` 下正常渐变在 `t=0.5` 的落盘值是 **181**，而退化色仍是 **128**。
   - 合成（blend）发生在展开空间里，所以退化色叠到非黑背景上时，落盘值也随 `colorspace` 变化。

   实测（本文三色标，退化 `Repeat`）：

   | colorspace | 叠黑底 | 叠 50% 灰 | 叠白底 |
   | --- | --- | --- | --- |
   | `Linear` | `(50,57,52)` | `(105,112,107)` | `(160,167,162)` |
   | `Gamma2` | `(66,75,69)` | `(121,131,125)` | `(176,185,179)` |
   | `SimpleSRGB` | `(68,77,71)` | `(123,132,126)` | `(178,187,181)` |
2. **alpha 不保证"看不见"**：`Pad` 退化保留末色标的 alpha。末色标 `a=64` → 覆盖 25%；末色标 `a=0` → 真的什么都不画（实测白底保持 255）。
3. **单色标 / 空色标**：单色标直接 `SolidColor`（含其 alpha）；空返回 `None`（不画）。
4. **色标位置乱序可能导致 panic**：`average_gradient_color` 最后用 `Color::from_rgba(...).unwrap()`（`linear_gradient.rs:121`）。它假设色标按位置升序、权重非负；乱序时权重会变负，加权和可能跑出 `[0,1]`，`from_rgba` 返回 `None` → **panic**（release 下同样 panic，实测 `(0.8,0.2,0.5)` + 高饱和颜色会在 `linear_gradient.rs:121` 崩溃）。而位置相同的多个色标是安全的（实测 `0.5/0.5` 得到两色 50/50 平均）。
5. **位置会被 clamp**：`GradientStop::new` 用 `NormalizedF32::new_clamped`，`<0` → 0，`>1` → 1，NaN/inf → 0。所以"位置 5.0"实际是 1.0，会参与 `w_n` 的计算而不是报错。
6. **`Repeat` 与 `Reflect` 在这个分支里完全等价**（`linear_gradient.rs:73`）：周期为 0 时"取绝对值镜像"和"直接取模"对 `t` 的后续查找没有区别；两者的差异只在非退化时出现。

---

## 6. 给美术工具的建议

1. **要"纯色"就用纯色**，不要依赖渐变塌缩来制造实色；`Pad` 的塌缩色是末色标，不是中点，也不是平均。
2. **要"淡出到无"**：用 `Pad` 并把末色标设成目标透明度（塌缩后正好是该 alpha），或直接改 paint 的 `apply_opacity`；不要靠把两端点写到一个极小的距离上。
3. **避免把缩放烘进色标坐标**。图标缩小应在绘制变换里做；烘焙只会在极端缩放下把局部间距压到 `3e-5` 以下，从而从"窄色带"跳成"整块平色"。
4. **保持局部间距 `≥ 1e-3` 个单位**（留 30 倍以上余量），并避免色标绝对坐标过大（见 §4.2 的量化表）。若必须在 0..1 的 viewBox 里画，间距也别低于 1e-3。
5. **色标必须按位置升序导出**，否则在退化时可能 panic（§5.4），这是工具侧最值得加的校验。
6. **透明度不要藏在透明色标里**：`Repeat`/`Reflect` 的平均会把这些 RGB 计进去（见 §2.2 尾注），色相凭空偏移，且与"逐色标预乘后平均"不同。若工具内部以预乘颜色建模，请在导出前明确目标语义。
7. **像素级夹具要么固定局部长度远离阈值，要么显式使用 `SolidColor`**，否则 f32 量化 + lowp/highp ±1 会让快照不稳定。

---

## 7. 复现脚本

在仓库根目录运行（临时示例文件，跑完可删）：

```rust
use tiny_skia::*;

fn main() {
    let stops = || vec![
        GradientStop::new(0.10, Color::from_rgba8(200, 40, 40, 255)),
        GradientStop::new(0.50, Color::from_rgba8( 40, 200, 40, 128)),
        GradientStop::new(0.85, Color::from_rgba8( 40,  40, 200, 64)),
    ];

    for (name, mode) in [("Pad", SpreadMode::Pad), ("Repeat", SpreadMode::Repeat)] {
        // 局部长度 1e-6 << 3.0517578e-5 -> 退化
        let sh = LinearGradient::new(
            Point::from_xy(0.0, 0.0), Point::from_xy(1e-6, 0.0),
            stops(), mode, Transform::identity(),
        ).unwrap();
        if let Shader::SolidColor(c) = &sh {
            println!("{name}: solid = {:?}", c.to_color_u8());   // 未预乘
        }

        let mut paint = Paint::default();
        paint.anti_alias = false;
        paint.shader = sh;
        for (bg_name, bg) in [("white", Color::WHITE), ("gray128", Color::from_rgba8(128,128,128,255))] {
            let mut pm = Pixmap::new(1, 1).unwrap();
            pm.fill(bg);
            pm.fill_rect(Rect::from_ltrb(0.0, 0.0, 1.0, 1.0).unwrap(),
                         &paint, Transform::identity(), None);
            let p = pm.pixel(0, 0).unwrap();
            println!("  {bg_name}: ({},{},{},{})", p.red(), p.green(), p.blue(), p.alpha());
        }
    }
}
```

预期输出（`to_color_u8()` 打的是**未预乘**色；`pixel()` 返回预乘后的存盘值）：

```
Pad: solid = ColorU8 { r: 40, g: 40, b: 200, a: 64 }
  white:   (201,201,241,255)
  gray128: (106,106,146,255)
Repeat: solid = ColorU8 { r: 88, g: 100, b: 92, a: 145 }
  white:   (160,167,162,255)
  gray128: (105,112,107,255)
```

---

## 8. 源码索引

| 内容 | 位置 |
| --- | --- |
| 阈值常量 `1/32768` | `src/shaders/gradient.rs:18` |
| 退化判定 / `Pad` 取末色标 / 平均 | `src/shaders/linear_gradient.rs:50-80` |
| 加权平均实现（含首尾隐式区间） | `src/shaders/linear_gradient.rs:112-162` |
| 单色标短路 | `src/shaders/linear_gradient.rs:46-48` |
| `SolidColor` 预乘 + 推入管线 | `src/shaders/mod.rs:83-86` |
| `Shader::transform` 对实色是空操作 | `src/shaders/mod.rs:96-112` |
| 非退化时颜色空间展开与逐段 factor/bias | `src/shaders/gradient.rs:127-250` |
| 绘制变换作用于 shader | `src/painter.rs:302-313` |
| `push_uniform_color` 的 u16 舍入 | `src/pipeline/mod.rs:380-396` |
| lowp 合成 + `div255` | `src/pipeline/lowp.rs:534-541`、`849-854` |
| highp 合成 + 归一化舍入 | `src/pipeline/highp.rs:669-679`、`1342-1344` |
| `Color::premultiply` / `demultiply` / `to_color_u8` | `src/color.rs:338-356`、`160-173`、`357-360` |
| 色彩空间展开 | `src/color.rs:480-502` |
| `is_nearly_zero_within_tolerance` | `path/src/scalar.rs:68-71` |
| `Point::length` | `path/src/lib.rs:150-158` |
| `Transform::invert` 的奇异/非有限处理 | `path/src/transform.rs:296-335` |
