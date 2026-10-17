// Each thread processes one pattern.
//
// Partial likelihoods are stored per internal node:
// ((node - NUM_LEAVES) * 2 + selector) * NUM_PATTERNS + pattern

__device__ f64x4 hadamard(const f64x4 a, const f64x4 b) {
	return make_f64x4(a.x * b.x, a.y * b.y, a.z * b.z, a.w * b.w);
}

__device__ f64 dot(const f64x4 a, const f64x4 b) {
	return a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w;
}

// Row `row` of matrix `half` (0 = left, 1 = right) of edge `i`
#define MATROW(i, half, row) \
	transitions[((i) * 2 + (half)) * 4 + (row)]

#define lidx(node, sel) \
	(((node) - NUM_LEAVES) * 2 + (sel)) * NUM_PATTERNS + pattern

#define PATTERN_PRELUDE \
	u32 pattern = blockIdx.x * blockDim.x + threadIdx.x; \
	if (pattern >= NUM_PATTERNS) { \
		return; \
	} \

#define SAMPLE_VALUE(sample, tv) \
	((sample) == 0 ? (tv).x \
	: (sample) == 1 ? (tv).y \
	: (sample) == 2 ? (tv).z \
	: (sample) == 3 ? (tv).w \
	: 1.0)

// Scale `prod` like Cpu4, tracking changes relative to the old slot.
#define APPLY_SCALE(old_scale) { \
	if (prod.x < SCALE_THRESHOLD \
		&& prod.y < SCALE_THRESHOLD \
		&& prod.z < SCALE_THRESHOLD \
		&& prod.w < SCALE_THRESHOLD) \
	{ \
		prod.x *= SCALE_MULT; \
		prod.y *= SCALE_MULT; \
		prod.z *= SCALE_MULT; \
		prod.w *= SCALE_MULT; \
		if (old_scale == 0) { \
			scale_sum += 1; \
		} \
		scale = 1; \
	} else { \
		if (old_scale != 0) { \
			scale_sum -= 1; \
		} \
		scale = 0; \
	} \
}

entrypoint __launch_bounds__(128)
void propose(
	const u8* __restrict__ leaves,
	f64x4* __restrict__ partials,
	u8* __restrict__ scales,
	u32* __restrict__ scale_sums,
	const u8* __restrict__ selectors,

	const u32 num_internals,
	const u32* __restrict__ internals,
	const uint2* __restrict__ children,
	const f64x4* __restrict__ transitions
) {
	PATTERN_PRELUDE

	u32 scale_sum = scale_sums[pattern];

	for (u32 i = 0; i < num_internals; i++) {
		u32 node = internals[i];
		uint2 kids = children[i];

		f64x4 prod;
		u8 scale;
		if (kids.y < NUM_LEAVES) {
			u8 sample_l = leaves[kids.x * NUM_PATTERNS + pattern];
			u8 sample_r = leaves[kids.y * NUM_PATTERNS + pattern];

			prod.x = SAMPLE_VALUE(sample_l, MATROW(i, 0, 0)) * SAMPLE_VALUE(sample_r, MATROW(i, 1, 0));
			prod.y = SAMPLE_VALUE(sample_l, MATROW(i, 0, 1)) * SAMPLE_VALUE(sample_r, MATROW(i, 1, 1));
			prod.z = SAMPLE_VALUE(sample_l, MATROW(i, 0, 2)) * SAMPLE_VALUE(sample_r, MATROW(i, 1, 2));
			prod.w = SAMPLE_VALUE(sample_l, MATROW(i, 0, 3)) * SAMPLE_VALUE(sample_r, MATROW(i, 1, 3));

			if (scales[lidx(node, selectors[node - NUM_LEAVES] & 1 ^ 1)] != 0) {
				scale_sum -= 1;
			}
			scale = 0;
		} else if (kids.x < NUM_LEAVES) {
			u8 sample_l = leaves[kids.x * NUM_PATTERNS + pattern];
			f64x4 right = partials[lidx(kids.y, selectors[kids.y - NUM_LEAVES] & 1)];

			f64 dr = dot(MATROW(i, 1, 0), right);
			prod.x = SAMPLE_VALUE(sample_l, MATROW(i, 0, 0)) * dr;
			dr = dot(MATROW(i, 1, 1), right);
			prod.y = SAMPLE_VALUE(sample_l, MATROW(i, 0, 1)) * dr;
			dr = dot(MATROW(i, 1, 2), right);
			prod.z = SAMPLE_VALUE(sample_l, MATROW(i, 0, 2)) * dr;
			dr = dot(MATROW(i, 1, 3), right);
			prod.w = SAMPLE_VALUE(sample_l, MATROW(i, 0, 3)) * dr;

			if (scales[lidx(node, selectors[node - NUM_LEAVES] & 1 ^ 1)] != 0) {
				scale_sum -= 1;
			}
			scale = 0;
		} else {
			f64x4 left = partials[lidx(kids.x, selectors[kids.x - NUM_LEAVES] & 1)];
			f64x4 right = partials[lidx(kids.y, selectors[kids.y - NUM_LEAVES] & 1)];

			f64 dl = dot(MATROW(i, 0, 0), left);
			f64 dr = dot(MATROW(i, 1, 0), right);

			prod.x = dl * dr;
			dl = dot(MATROW(i, 0, 1), left);
			dr = dot(MATROW(i, 1, 1), right);
			prod.y = dl * dr;
			dl = dot(MATROW(i, 0, 2), left);
			dr = dot(MATROW(i, 1, 2), right);
			prod.z = dl * dr;
			dl = dot(MATROW(i, 0, 3), left);
			dr = dot(MATROW(i, 1, 3), right);
			prod.w = dl * dr;

			APPLY_SCALE(scales[lidx(node, selectors[node - NUM_LEAVES] & 1 ^ 1)])
		}

		scales[lidx(node, selectors[node - NUM_LEAVES] & 1)] = scale;
		partials[lidx(node, selectors[node - NUM_LEAVES] & 1)] = prod;
	}

	scale_sums[pattern] = scale_sum;
}

entrypoint __launch_bounds__(128)
void update_likelihoods(
	const f64x4* __restrict__ partials,
	f64* __restrict__ likelihoods,
	const u8* __restrict__ selectors,
	u32 root,
	f64x4 frequencies
) {
	PATTERN_PRELUDE

	u32 root_sel = selectors[root - NUM_LEAVES] & 1;
	likelihoods[pattern] = log(dot(partials[lidx(root, root_sel)], frequencies));
}
