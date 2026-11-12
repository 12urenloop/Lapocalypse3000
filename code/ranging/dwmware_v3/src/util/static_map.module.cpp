
export module static_map;

export namespace ct {

using u64 = unsigned long long;
using u32 = unsigned int;

constexpr u64 hash(u64 x, u64 seed) noexcept {
  x += seed + 0x9E3779B97F4A7C15ULL;
  x = (x ^ (x >> 30)) * 0xBF58476D1CE4E5B9ULL;
  x = (x ^ (x >> 27)) * 0x94D049BB133111EBULL;
  return x ^ (x >> 31);
}

template <typename Key, typename Value> struct Entry {
  Key key;
  Value value;
};

template <typename Key, typename Value, u32 N, typename Index = unsigned short>
class StaticMap {
  static_assert(N > 0);
  static_assert(N < static_cast<u64>(static_cast<Index>(-1)));
  static_assert(N <= 0x3FFFFFFFu);

  static constexpr u32 capacity = 4u * N;
  static constexpr u32 max_seeds = 4096;

  Entry<Key, Value> entries_[N]{};

  // Primary hash metadata.
  u32 offsets_[N]{};
  u32 sizes_[N]{};
  u64 seeds_[N]{};

  // Shared secondary storage pool.
  Index slots_[capacity]{};

  u64 primary_seed_ = 0;

  constexpr u32 primary(Key key, u64 seed) const noexcept {
    return hash(static_cast<u64>(key), seed) % N;
  }

  constexpr u32 secondary(Key key, u64 seed, u32 size) const noexcept {
    return hash(static_cast<u64>(key), seed) % size;
  }

public:
  consteval StaticMap(const Entry<Key, Value> (&entries)[N]) {
    for (u32 i = 0; i < N; ++i)
      entries_[i] = entries[i];

    u32 counts[N]{};
    bool found = false;

    // Find primary hash with bounded secondary storage.
    for (u64 seed = 0; seed < max_seeds; ++seed) {
      for (u32 i = 0; i < N; ++i)
        counts[i] = 0;

      for (u32 i = 0; i < N; ++i)
        ++counts[primary(entries_[i].key, seed)];

      u32 total = 0;

      for (u32 i = 0; i < N; ++i)
        total += counts[i] * counts[i];

      if (total <= capacity) {
        primary_seed_ = seed;
        found = true;
        break;
      }
    }

    if (!found) {
#ifdef __cpp_exceptions
      throw "StaticMap: primary hash search failed";
#else
      // Force a constant-evaluation failure when exceptions are disabled.
      // Division by zero is not permitted in a constant expression.
      (void)(1 / 0);
#endif
    }

    // Calculate secondary table offsets.
    u32 offset = 0;

    for (u32 i = 0; i < N; ++i) {
      offsets_[i] = offset;
      sizes_[i] = counts[i] * counts[i];
      offset += sizes_[i];
    }

    // Construct collision-free secondary tables.
    for (u32 bucket = 0; bucket < N; ++bucket) {
      const u32 size = sizes_[bucket];

      if (size == 0)
        continue;

      bool bucket_found = false;

      for (u64 seed = 0; seed < max_seeds; ++seed) {
        bool collision = false;

        for (u32 i = 0; i < N && !collision; ++i) {
          if (primary(entries_[i].key, primary_seed_) != bucket)
            continue;

          const u32 a = secondary(entries_[i].key, seed, size);

          for (u32 j = 0; j < i; ++j) {
            if (primary(entries_[j].key, primary_seed_) != bucket)
              continue;

            if (a == secondary(entries_[j].key, seed, size)) {
              collision = true;
              break;
            }
          }
        }

        if (!collision) {
          seeds_[bucket] = seed;
          bucket_found = true;
          break;
        }
      }

      if (!bucket_found) {
#ifdef __cpp_exceptions
        throw "StaticMap: secondary hash search failed";
#else
        // Force a constant-evaluation failure when exceptions are disabled.
        // Division by zero is not permitted in a constant expression.
        (void)(1 / 0);
#endif
      }

      // Insert entries into shared storage.
      for (u32 i = 0; i < N; ++i) {
        if (primary(entries_[i].key, primary_seed_) != bucket)
          continue;

        const u32 slot = secondary(entries_[i].key, seeds_[bucket], size);

        slots_[offsets_[bucket] + slot] = static_cast<Index>(i + 1);
      }
    }
  }

  [[nodiscard]]
  constexpr const Value *find(Key key) const noexcept {
    const u32 bucket = primary(key, primary_seed_);
    const u32 size = sizes_[bucket];

    if (size == 0)
      return nullptr;

    const u32 slot = secondary(key, seeds_[bucket], size);

    const Index entry_index = slots_[offsets_[bucket] + slot];

    if (entry_index == 0)
      return nullptr;

    const auto &entry = entries_[entry_index - 1];

    return entry.key == key ? &entry.value : nullptr;
  }

  [[nodiscard]]
  constexpr bool contains(Key key) const noexcept {
    return find(key) != nullptr;
  }

  [[nodiscard]]
  constexpr u32 size() const noexcept {
    return N;
  }
};

template <typename Key, typename Value, u32 N>
consteval auto make_static_map(const Entry<Key, Value> (&entries)[N]) {
  return StaticMap<Key, Value, N>(entries);
}

} // namespace ct
