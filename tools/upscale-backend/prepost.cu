// GPU pre/post processing for the validated V3 tile contract. No Torch dependency.
__device__ int bound(int v, int hi) { return v < 0 ? 0 : (v >= hi ? hi - 1 : v); }
__device__ float clip(float v) { return fminf(fmaxf(v, 0.0f), 1.0f); }

extern "C" __global__ void inspect_alpha(const float* alpha, int* flag, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) {
        if (alpha[i] != 1.0f) atomicOr(flag, 1);
        if (!isfinite(alpha[i])) atomicOr(flag, 2);
    }
}

extern "C" __global__ void white_rgb(const float* rgb, const float* alpha, float* out, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < 3 * n) { float a = alpha[i % n]; out[i] = rgb[i] * a + (1.0f - a); }
}

extern "C" __global__ void border_init(const float* rgb, const float* alpha, float* out, float* mask, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) {
        float valid = alpha[i] > 0.0f ? 1.0f : 0.0f;
        mask[i] = valid;
        for (int c = 0; c < 3; ++c) out[c * n + i] = valid ? rgb[c * n + i] : 0.0f;
    }
}

extern "C" __global__ void border_step(const float* rgb, const float* mask, float* out, float* next, int h, int w, int last) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    int n = h * w;
    if (i >= n) return;
    int y = i / w, x = i % w;
    float weight = 0.0f, sums[3] = {0.0f, 0.0f, 0.0f};
    for (int dy = -1; dy <= 1; ++dy) for (int dx = -1; dx <= 1; ++dx) {
        int yy = y + dy, xx = x + dx;
        if (yy >= 0 && yy < h && xx >= 0 && xx < w) {
            int j = yy * w + xx;
            weight += mask[j];
            for (int c = 0; c < 3; ++c) sums[c] += rgb[c * n + j];
        }
    }
    next[i] = weight > 0.0f ? 1.0f : 0.0f;
    for (int c = 0; c < 3; ++c) {
        float v = mask[i] < 1.0f ? sums[c] / (weight + 1e-7f) : rgb[c * n + i];
        out[c * n + i] = last ? clip(v) : v;
    }
}

extern "C" __global__ void gather_tiles(const float* source, float* feed,
        int h, int w, int th, int tw, int tile, int step, int nw, int start,
        int count, int rotate, int horizontal, int vertical, int alpha_source) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    int area = tile * tile;
    if (i >= count * 3 * area) return;
    int batch = i / (3 * area), channel = (i / area) % 3, p = i % area;
    int index = start + batch;
    int ty = bound((index / nw) * step + p / tile - 8, th);
    int tx = bound((index % nw) * step + p % tile - 8, tw);
    if (vertical) ty = th - 1 - ty;
    if (horizontal) tx = tw - 1 - tx;
    int sy = rotate ? tx : ty;
    int sx = rotate ? w - 1 - ty : tx;
    feed[i] = source[(alpha_source ? 0 : channel * h * w) + sy * w + sx];
}

extern "C" __global__ void merge_tile(const float* output, float* pixels, float* weights, int* flag,
        int side, int ph, int pw, int top, int left, int batch) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i >= side * side) return;
    int y = i / side, x = i % side;
    int edge = min(min(y + 1, side - y), min(x + 1, side - x));
    float weight = (float)min(edge, 9) / 9.0f;
    int pos = (top + y) * pw + left + x;
    float old = weights[pos], ratio = old / (old + weight);
    for (int c = 0; c < 3; ++c) {
        float v = output[(batch * 3 + c) * side * side + i];
        if (!isfinite(v)) atomicOr(flag, 2);
        int dst = c * ph * pw + pos;
        pixels[dst] = pixels[dst] * ratio + v * (1.0f - ratio);
    }
    weights[pos] = old + weight;
}

extern "C" __global__ void tta_add(const float* pixels, float* average,
        int oh, int ow, int ph, int pw, int rotate, int horizontal, int vertical, int first) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    int n = oh * ow;
    if (i >= n) return;
    int y = i / ow, x = i % ow;
    int th = rotate ? ow : oh, tw = rotate ? oh : ow;
    int ty = rotate ? ow - 1 - x : y, tx = rotate ? y : x;
    if (vertical) ty = th - 1 - ty;
    if (horizontal) tx = tw - 1 - tx;
    for (int c = 0; c < 3; ++c) {
        float v = clip(pixels[c * ph * pw + ty * pw + tx]);
        int dst = c * n + i;
        average[dst] = first ? v : average[dst] + v;
    }
}

extern "C" __global__ void finish_rgb(float* rgb, int n, int variants) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) rgb[i] = clip(rgb[i] / (float)variants);
}

extern "C" __global__ void finish_alpha(const float* pixels, float* alpha, int oh, int ow, int ph, int pw, int opaque) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i >= oh * ow) return;
    int j = (i / ow) * pw + i % ow;
    alpha[i] = opaque ? 1.0f : (clip(pixels[j]) + clip(pixels[ph * pw + j]) + clip(pixels[2 * ph * pw + j])) / 3.0f;
}

extern "C" __global__ void gather_upscale(const float* source, float* feed, int h, int w,
        int th, int tw, int top, int left) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i >= 3 * th * tw) return;
    int c = i / (th * tw), p = i % (th * tw);
    int y = bound(top + p / tw, h), x = bound(left + p % tw, w);
    feed[i] = source[c * h * w + y * w + x];
}

extern "C" __global__ void merge_upscale(const float* tile, float* out, float* weights, int* flag,
        int oh, int ow, int th, int tw, int top, int left, int overlap, int has_top, int has_left,
        int has_bottom, int has_right) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i >= th * tw) return;
    int y = i / tw, x = i % tw, yy = top + y, xx = left + x;
    if (yy >= oh || xx >= ow) return;
    int blend = min(overlap, 64), cut = (overlap - blend) / 2;
    float wy = 1.0f, wx = 1.0f;
    if (has_top) wy = fminf(wy, (float)(y - cut + 1) / (blend + 1));
    if (has_bottom) wy = fminf(wy, (float)(th - y - cut) / (blend + 1));
    if (has_left) wx = fminf(wx, (float)(x - cut + 1) / (blend + 1));
    if (has_right) wx = fminf(wx, (float)(tw - x - cut) / (blend + 1));
    float weight = fmaxf(0.0f, fminf(1.0f, wy)) * fmaxf(0.0f, fminf(1.0f, wx));
    if (weight == 0.0f) return;
    int p = yy * ow + xx; float old = weights[p], ratio = old / (old + weight);
    for (int c = 0; c < 3; ++c) {
        float v = tile[c * th * tw + i];
        if (!isfinite(v)) atomicOr(flag, 2);
        out[c * oh * ow + p] = out[c * oh * ow + p] * ratio + v * (1.0f - ratio);
    }
    weights[p] = old + weight;
}

extern "C" __global__ void resize_alpha(const float* in, float* out, int h, int w, int scale) {
    int i = blockIdx.x * blockDim.x + threadIdx.x, ow = w * scale, oh = h * scale;
    if (i >= ow * oh) return;
    float yf = ((float)(i / ow) + 0.5f) / scale - 0.5f;
    float xf = ((float)(i % ow) + 0.5f) / scale - 0.5f;
    int y = (int)floorf(yf), x = (int)floorf(xf); float fy = yf - y, fx = xf - x;
    float a = in[bound(y,h)*w+bound(x,w)], b = in[bound(y,h)*w+bound(x+1,w)];
    float c = in[bound(y+1,h)*w+bound(x,w)], d = in[bound(y+1,h)*w+bound(x+1,w)];
    out[i] = clip((a*(1-fx)+b*fx)*(1-fy)+(c*(1-fx)+d*fx)*fy);
}
