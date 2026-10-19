__device__ inline f64 dot(const f64x4 a, const f64x4 b) {
	return a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w;
}

__constant__ f64x4 leaf_partials[] = {
	{1.0, 0.0, 0.0, 0.0},
	{0.0, 1.0, 0.0, 0.0},
	{0.0, 0.0, 1.0, 0.0},
	{0.0, 0.0, 0.0, 1.0},
	{1.0, 1.0, 1.0, 1.0},
};

// Leaf index
#define lidx(node) \
	(node) * NUM_PATTERNS + pattern

#define sidx(node) \
	((node - NUM_LEAVES) * 2 + (selectors[node - NUM_LEAVES] & 1)) * NUM_PATTERNS + pattern

#define soidx(node) \
	((node - NUM_LEAVES) * 2 + (selectors[node - NUM_LEAVES] & 1 ^ 1)) * NUM_PATTERNS + pattern

#define tv_leaf(sample, tv) \
	((sample) < 4 ? (&(tv).x)[sample] : 1.0)

#define trow(edge, row) \
	transitions[(edge) * 4 + (row)]

// Transitioned child likelihoods, componentwise product
#define calc_likelihood(left, right) \
	make_f64x4( \
		(left) * (right), \
	...)


entrypoint __launch_bounds__(BLOCK_SIZE)
void propose(
	const u8* __restrict__ leaves,
	f64x4* __restrict__ partials,
	u8* __restrict__ scales,
	u32* __restrict__ scale_sums,
	const u8* __restrict__ selectors,

	const u32 num_internals,
	const u32* __restrict__ internals,
	const u32* __restrict__ children,
	const f64x4* __restrict__ transitions,
	f64* __restrict__ likelihoods,
	f64x4 frequencies
) {
	u32 pattern = blockIdx.x * blockDim.x + threadIdx.x;
	if (pattern >= NUM_PATTERNS) {
		return;
	}

	u32 scale_sum = scale_sums[pattern];

	f64x4 likelihood;

	for (u32 i = 0; i < num_internals; i++) {
		u32 node = internals[i];
		u32 left = children[i * 2];
		u32 right = children[i * 2 + 1];

		u8 old_scale = scales[soidx(node)];

		if (right < NUM_LEAVES) {
			u8 leaf_left = leaves[lidx(left)];
			u8 leaf_right = leaves[lidx(right)];

			likelihood = make_f64x4(
				tv_leaf(leaf_left, trow(i * 2, 0))
					* tv_leaf(leaf_right, trow(i * 2 + 1, 0)),
				tv_leaf(leaf_left, trow(i * 2, 1))
					* tv_leaf(leaf_right, trow(i * 2 + 1, 1)),
				tv_leaf(leaf_left, trow(i * 2, 2))
					* tv_leaf(leaf_right, trow(i * 2 + 1, 2)),
				tv_leaf(leaf_left, trow(i * 2, 3))
					* tv_leaf(leaf_right, trow(i * 2 + 1, 3))
			);
		} else if (left < NUM_LEAVES) {
			u8 leaf_left = leaves[lidx(left)];
			f64x4 partial_right = partials[sidx(right)];

			likelihood = make_f64x4(
				tv_leaf(leaf_left, trow(i * 2, 0))
					* dot(trow(i * 2 + 1, 0), partial_right),
				tv_leaf(leaf_left, trow(i * 2, 1))
					* dot(trow(i * 2 + 1, 1), partial_right),
				tv_leaf(leaf_left, trow(i * 2, 2))
					* dot(trow(i * 2 + 1, 2), partial_right),
				tv_leaf(leaf_left, trow(i * 2, 3))
					* dot(trow(i * 2 + 1, 3), partial_right)
			);
		} else {
			f64x4 partial_left = partials[sidx(left)];
			f64x4 partial_right = partials[sidx(right)];

			likelihood = make_f64x4(
				dot(trow(i * 2, 0), partial_left)
					* dot(trow(i * 2 + 1, 0), partial_right),
				dot(trow(i * 2, 1), partial_left)
					* dot(trow(i * 2 + 1, 1), partial_right),
				dot(trow(i * 2, 2), partial_left)
					* dot(trow(i * 2 + 1, 2), partial_right),
				dot(trow(i * 2, 3), partial_left)
					* dot(trow(i * 2 + 1, 3), partial_right)
			);
		}

		u32 should_scale = likelihood.x < SCALE_THRESHOLD
			&& likelihood.y < SCALE_THRESHOLD
			&& likelihood.z < SCALE_THRESHOLD
			&& likelihood.w < SCALE_THRESHOLD;

		if (should_scale) {
			likelihood.x *= SCALE_MULT;
			likelihood.y *= SCALE_MULT;
			likelihood.z *= SCALE_MULT;
			likelihood.w *= SCALE_MULT;
		}

		scales[sidx(node)] = should_scale;
		if (old_scale == 0 && should_scale == 1) {
			scale_sum += SCALE_LN;
		} else if (old_scale == 1 && should_scale == 0) {
			scale_sum -= SCALE_LN;
		}

		partials[sidx(node)] = likelihood;
	}


	scale_sums[pattern] = scale_sum;

	u32 root = internals[num_internals - 1];
	f64x4 root_partial = partials[sidx(root)];
	f64 likelihood_sum = log(dot(root_partial, frequencies)) - scale_sum;
	likelihoods[pattern] = likelihood_sum;
}
