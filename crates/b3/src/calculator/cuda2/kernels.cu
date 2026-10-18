#include <cooperative_groups.h>

using namespace cooperative_groups;


__device__ inline f64 dot(const f64x4 a, const f64x4 b) {
	return a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w;
}

__device__ inline f64x4 get_leaf_partial(u8 leaf) {
	switch (leaf) {
		case 1:  return make_f64x4(1.0, 0.0, 0.0, 0.0);
		case 2:  return make_f64x4(0.0, 1.0, 0.0, 0.0);
		case 4:  return make_f64x4(0.0, 0.0, 1.0, 0.0);
		case 8:  return make_f64x4(0.0, 0.0, 0.0, 1.0);
		default: return make_f64x4(1.0, 1.0, 1.0, 1.0);
	}
}

// Leaf index
#define lidx(node) \
	(node) * NUM_PATTERNS + pattern

#define sidx(node) \
	((node - NUM_LEAVES) * 2 + (selectors[node - NUM_LEAVES] & 1)) * NUM_PATTERNS + pattern

#define soidx(node) \
	((node - NUM_LEAVES) * 2 + (selectors[node - NUM_LEAVES] & 1 ^ 1)) * NUM_PATTERNS + pattern

#define spidx(node) \
	(sidx(node)) * 4 + sub


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
	u32 pattern = (blockIdx.x * blockDim.x + threadIdx.x) / 4;
	if (pattern >= NUM_PATTERNS) {
		return;
	}
	auto g = tiled_partition<4>(this_thread_block());
	u32 sub = g.thread_rank();
	u32 tile = threadIdx.x / 4;

	__shared__ f64 s_likelihood[BLOCK_SIZE];

	u32 scale_sum = scale_sums[pattern];

	for (u32 i = 0; i < num_internals; i++) {
		u32 node = internals[i];
		u32 left = children[i * 2];
		u32 right = children[i * 2 + 1];

		f64x4 tv_left = transitions[i * 2 * 4 + sub];
		f64x4 tv_right = transitions[(i * 2 + 1) * 4 + sub];

		u8 old_scale = scales[soidx(node)];

		f64x4 partial_left, partial_right;

		if (right < NUM_LEAVES) {
			partial_left = get_leaf_partial(leaves[lidx(left)]);
			partial_right = get_leaf_partial(leaves[lidx(right)]);
		} else if (left < NUM_LEAVES) {
			partial_left = get_leaf_partial(leaves[lidx(left)]);
			partial_right = partials[sidx(right)];
		} else {
			partial_left = partials[sidx(left)];
			partial_right = partials[sidx(right)];
		}

		f64 l_likelihood = dot(tv_left, partial_left) * dot(tv_right, partial_right);

		s_likelihood[tile * 4 + sub] = l_likelihood;
		g.sync();

		u32 should_scale = s_likelihood[tile * 4 + 0] < SCALE_THRESHOLD
			&& s_likelihood[tile * 4 + 1] < SCALE_THRESHOLD
			&& s_likelihood[tile * 4 + 2] < SCALE_THRESHOLD
			&& s_likelihood[tile * 4 + 3] < SCALE_THRESHOLD;

		if (should_scale) {
			l_likelihood *= SCALE_MULT;
			g.sync();
		}

		if (sub == 0) {
			scales[sidx(node)] = should_scale;
			if (old_scale == 0 && should_scale == 1) {
				scale_sum += SCALE_LN;
			} else if (old_scale == 1 && should_scale == 0) {
				scale_sum -= SCALE_LN;
			}
		}

		f64* partials_ptr = reinterpret_cast<f64*>(partials);
		partials_ptr[spidx(node)] = l_likelihood;
	}


	if (sub == 0) {
		scale_sums[pattern] = scale_sum;

		u32 root = internals[num_internals - 1];
		f64x4 root_partial = partials[sidx(root)];
		f64 likelihood = log(dot(root_partial, frequencies)) - scale_sum;
		likelihoods[pattern] = likelihood;
	}
}
