## Schema overview

Use two core tables: nodes (shared metadata for files and directories) and data_blobs (file contents, optional). Represent hierarchy with a parent_id FK (adjacency list) and add a materialized path and optional nested-set or closure table for fast queries. Include hashes and timestamps for dedup, integrity, and incremental updates.

### nodes (single-table for files and directories)
- id INTEGER PRIMARY KEY
- parent_id INTEGER NULL REFERENCES nodes(id) ON DELETE CASCADE
- name TEXT NOT NULL  -- basename
- type TEXT NOT NULL CHECK(type IN ('file','dir','symlink','device','fifo','socket')) 
- mode INTEGER NOT NULL  -- POSIX perms
- uid INTEGER NULL
- gid INTEGER NULL
- size INTEGER NOT NULL DEFAULT 0
- mtime INTEGER NOT NULL  -- epoch seconds (or use ISO8601 TEXT)
- ctime INTEGER NOT NULL
- atime INTEGER NULL
- inode INTEGER NULL  -- original FS inode (optional)
- dev INTEGER NULL    -- device number (optional)
- link_target TEXT NULL  -- for symlinks
- content_blob_id INTEGER NULL REFERENCES data_blobs(id)  -- for files stored inline via blobs
- content_hash TEXT NULL  -- e.g., sha256 for dedup/verification
- flags INTEGER DEFAULT 0  -- user-defined bitflags
- is_hardlink BOOLEAN DEFAULT 0
- hardlink_group TEXT NULL  -- same value for hardlinked files (e.g., device:inode)
- path TEXT NOT NULL  -- materialized absolute path (kept for convenience & snapshots)
- depth INTEGER NOT NULL DEFAULT 0
- created_at INTEGER DEFAULT (strftime('%s','now'))  -- record timestamp

Indexes: UNIQUE(parent_id,name) to enforce single child name per directory; index on path; index on content_hash; index on (parent_id, name).

### data_blobs (store file contents, optional)
- id INTEGER PRIMARY KEY
- hash TEXT UNIQUE NOT NULL  -- sha256
- size INTEGER NOT NULL
- compression TEXT NULL -- e.g., 'zstd' or NULL
- blob BLOB NOT NULL
- created_at INTEGER DEFAULT (strftime('%s','now'))

This lets multiple nodes point to the same blob for dedup/hardlinks.

## Hierarchy representation options

1) Adjacency list (parent_id) --  simple, writable, minimal. Use recursive CTEs for traversal.
   - Good for: streaming ingestion, updates, space efficiency.
   - Query example (get subtree):
     ```
     WITH RECURSIVE sub(id) AS (
       SELECT id FROM nodes WHERE id = ?
       UNION ALL
       SELECT n.id FROM nodes n JOIN sub s ON n.parent_id = s.id
     )
     SELECT * FROM nodes WHERE id IN (SELECT id FROM sub);
     ```

2) Materialized path (path column) --  store absolute path string (e.g., "/usr/bin"). Fast prefix search, simple snapshotting.
   - Query subtree: SELECT * FROM nodes WHERE path LIKE '/usr/bin/%';
   - Keep path updated when renaming/moving (batch UPDATE of subtree).

3) Closure table -- explicit ancestor/descendant pairs for O(1) subtree queries at cost of extra writes and space.
   - closure (ancestor, descendant, depth)
   - Maintain on inserts/deletes; fast for queries and snapshots.

4) Nested sets (lft, rgt) -- fast subtree reads, expensive updates; not recommended for frequent mutations.

Recommendation: use adjacency list + materialized path + depth. Add closure table only if you need many repeated fast subtree queries with few writes.

## Handling special cases

- Hardlinks: set is_hardlink and hardlink_group (device:inode) and point content_blob_id to same blob.
- Symlinks: type='symlink' and link_target filled; size can be length of target.
- Permissions/owner differences: store mode/uid/gid per node.
- Devices/sockets: store major/minor in link_target or extra columns.
- Sparse files: store logical size in size and actual blob size in data_blobs.size; optionally store extents metadata.
- Large files: store blobs chunked (add blob_chunks table with blob_id, seq, data) to avoid single huge BLOB rows.
- Incremental updates: keep previous_snapshot_id and snapshot table to capture multiple snapshots without rewriting nodes (see next).

## Snapshots / multiple points-in-time

Option A -- full copy per snapshot:
- snapshots table (id, name, root_node_id, created_at)
- Duplicate nodes for each snapshot (path and parent pointers) -- simple but larger.

Option B -- copy-on-write:
- nodes.snapshot_id to tag nodes by snapshot; reuse node rows across snapshots where identical; create new nodes on change.

Option C -- immutable node entries + snapshot->root mapping:
- nodes are immutable; snapshot table points to root node id representing that snapshot.
- Use dedup of identical subtrees via content hashes and hardlink_group-like subtree fingerprints.

Recommendation: For a filesystem backup tool, use snapshots table + nodes with snapshot_id or immutable nodes with dedup by content_hash/subtree_hash depending on storage vs speed tradeoffs.

## Example minimal DDL (SQLite)

```
CREATE TABLE nodes (
  id INTEGER PRIMARY KEY,
  parent_id INTEGER REFERENCES nodes(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  type TEXT NOT NULL,
  mode INTEGER NOT NULL,
  uid INTEGER,
  gid INTEGER,
  size INTEGER NOT NULL DEFAULT 0,
  mtime INTEGER NOT NULL,
  ctime INTEGER NOT NULL,
  atime INTEGER,
  inode INTEGER,
  dev INTEGER,
  link_target TEXT,
  content_blob_id INTEGER REFERENCES data_blobs(id),
  content_hash TEXT,
  is_hardlink INTEGER DEFAULT 0,
  hardlink_group TEXT,
  path TEXT NOT NULL,
  depth INTEGER DEFAULT 0,
  snapshot_id INTEGER,
  created_at INTEGER DEFAULT (strftime('%s','now')),
  UNIQUE(parent_id, name)
);

CREATE INDEX idx_nodes_path ON nodes(path);
CREATE INDEX idx_nodes_parent ON nodes(parent_id);
CREATE INDEX idx_nodes_hash ON nodes(content_hash);

CREATE TABLE data_blobs (
  id INTEGER PRIMARY KEY,
  hash TEXT UNIQUE NOT NULL,
  size INTEGER NOT NULL,
  compression TEXT,
  blob BLOB NOT NULL,
  created_at INTEGER DEFAULT (strftime('%s','now'))
);
```

## Practical tips for Rust implementation
- Use rusqlite for transactions and prepared statements.
- Ingest with a single transaction per directory or snapshot to speed writes.
- Compute file content hash while reading; insert blob only if hash absent (dedup).
- Use recursive CTEs for queries; maintain path and depth on insert for fast reads.
- Use WAL mode and PRAGMA synchronous=NORMAL for speed; VACUUM/ANALYZE periodically.

