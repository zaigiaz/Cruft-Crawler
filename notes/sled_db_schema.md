# sled trees layout and how they work

## Trees
- **objects**  key: id (u128 or inode+dev tuple serialized) ? value: compact metadata blob  
  - Contains: parent_id, name (or name_hash), type, mtime, size, mode, flags, optional link_target, optional content_hash, schema_version.
- **path_index**  key: full_path (UTF-8) ? value: id  
  - One entry per path. For hard links, either:
    - store a separator-delimited list of ids (if id not inode-based)  not recommended, or
    - map path ? id where id is the stable inode+dev so multiple paths can map to same id.
- **name_index** (optional)  key: name_prefix_or_hash ? value: list/bitmap of ids or posting lists  
  - Used for fast name searches. Implement as prefix keys (e.g., "n:foo") pointing to a compact postings list (vector of ids) or to a bloom-filter/bitmap shard.
- **parent_index** (optional)  key: parent_id ? value: serialized list of child ids or small posting lists  
  - Speeds directory listings and incremental updates.
- **config/checkpoints**  key: "checkpoint" / "scan_cursor" / "schema" ? value: small JSON/binary

## Example key/value shapes
- objects tree:
  - key: 0x0001_... (u128 id)
  - value (bincode): { parent_id: u128, name_hash: u32, type: u8, mtime: i64, size: u64, mode: u32, flags: u8, link_target_offset: u32, content_hash: [u8;16]? }
- path_index tree:
  - key: "/home/alice/docs/report.pdf"
  - value: 0x0001...
- name_index tree (prefix approach):
  - key: "name:r:e:por" (prefix tokens or n-gram)
  - value: posting-list bytes (sorted u128 ids)

## How operations map to trees
- Insert new file discovered:
  1. Compute id (inode+dev or generated uuid).
  2. Write objects.insert(id, meta_blob).
  3. Write path_index.insert(full_path, id).
  4. Update parent_index.append(parent_id, id).
  5. If name_index enabled, update posting lists for name tokens.
  - Do above in one sled transactional batch to keep consistency.

- Rename / move:
  1. Lookup id = path_index.get(old_path).
  2. path_index.remove(old_path); path_index.insert(new_path, id).
  3. Update objects.update(id) to set parent_id and name if you store name in object.
  4. Update parent_index: remove from old_parent, add to new_parent.
  - Use a transaction for atomicity.

- Delete:
  1. path_index.remove(path).
  2. Option A: set objects.update(id, flags |= TOMBSTONE) (keep record for short retention).
  3. Update parent_index remove child.
  4. Option B (final prune): remove objects.delete(id) once all paths removed and tombstone expired.
  - Use transactions.

- Hard links:
  - Multiple path_index entries map to same id. objects.id remains single object. Track link count in objects if desired.

- Directory listing:
  - If parent_index exists: parent_index.get(parent_id) ? list child ids ? fetch objects for metadata (batched).
  - If parent_index absent: list by scanning path_index keys with prefix parent_path + "/" (works but slower).

- Name search:
  - Use name_index to get candidate ids quickly, then fetch objects to filter.

### Storage & performance considerations
- Keep objects compact; avoid storing full path in objects to reduce duplication.
- Posting lists in name_index should be compressed (delta + varint) and kept sorted for efficient merges/removals.
- Parent_index entries for very large dirs should be sharded (e.g., parent_id + shard_prefix) to keep value size bounded.
- Batch updates into a single sled transaction when modifying multiple trees to maintain consistency.
- For very large numbers of small updates, buffer and flush in batches to reduce IO.

### Failure & recovery
- Use sled transactions for atomic multi-tree updates. If transactions fail, reconciliation pass can:
  - Scan filesystem and repair path_index ? objects mismatches.
  - Remove orphaned objects or re-link them if inode found under different path.
- Keep schema version in config tree and include version in objects to apply migrations.



# Serialization
The serializer converts your in-memory Metadata struct into a compact byte sequence to store in sled (a key-value store that only stores bytes). Reasons:

- sled values are bytes: you must serialize structs to store them and deserialize when reading.
- Compactness: formats like bincode produce small, fast-to-encode bytes (less I/O).
- Stability/versioning: serialized records can include schema/version fields so you can migrate later.
- Interoperability: choosing a format (bincode/CBOR/msgpack) lets other tools read the DB if needed.
- Atomic writes: a single serialized blob means updating one value is atomic rather than juggling many separate keys.
- Performance: binary serializers are faster and smaller than text formats (JSON) for frequent reads/writes.

Choices:
- bincode:  compact, fast, Rust-native (no schema overhead).
- CBOR/msgpack:  compact, language-agnostic.
- JSON: human-readable but larger/slower.

So you need serialization because sled stores bytes; pick a compact, stable serializer that fits your cross-language and migration needs.
