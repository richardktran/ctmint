# Data Ingestion – Technical Design (ContextMint)

## 1. Purpose and Scope

The **Data Ingestion** pipeline extracts **database schema and usage metadata** and integrates it into the System Knowledge Graph (SKG). It provides:

- **Schema**: tables, columns, types, indexes, primary keys, foreign keys.
- **Usage mapping**: which services/functions read or write which tables (from code analysis and optionally from runtime or DB logs).
- **Context for AI**: When debugging “why is this slow?” or “what breaks if I change this column?”, the AI can traverse from Service/Endpoint/Function to DatabaseTable/Column and see indexes and relationships.

Data ingestion does **not** ingest row data; it only deals with **metadata** and **structural/usage information**.

---

## 2. High-Level Architecture

```text
                    Data Ingestion (this component)
                                │
        ┌───────────────────────┼───────────────────────┐
        │                       │                       │
   Schema Extractor        Usage Mapper           Optional: Slow-query
   (connect to DB,         (from Code Parser      /Audit log ingestion
    read catalog)          + optional runtime)    for table/column usage)
        │                       │                       │
        └───────────────────────┼───────────────────────┘
                                │
                        Normalized Schema Model
                    (Database, Table, Column, Index, FK)
                                │
                        SKG Writer + Optional Vector
```

---

## 3. Normalized Schema Model

All databases are mapped to a common schema for the SKG.

**Entities**:
- **Database**: logical database (e.g. one Postgres DB, one MySQL instance or schema).
- **DatabaseTable**: name, optional schema name (e.g. `public.users`).
- **Column**: name, type, nullable, default; belongs to Table.
- **Index**: name, columns, unique; belongs to Table.
- **PrimaryKey**: list of columns; belongs to Table.
- **ForeignKey**: from (table, column) to (table, column); belongs to Table.

**Relations to code** (from Code Parser / Indexer):
- `Function -[:READS]-> DatabaseTable`
- `Function -[:WRITES]-> DatabaseTable`
- Optional: `Function -[:QUERIES]-> DatabaseTable` with snippet or query pattern.

**Relations from schema**:
- `Database -[:CONTAINS]-> DatabaseTable`
- `DatabaseTable -[:HAS_COLUMN]-> Column`
- `DatabaseTable -[:HAS_INDEX]-> Index`
- `DatabaseTable -[:HAS_PRIMARY_KEY]-> PrimaryKey`
- `DatabaseTable -[:FOREIGN_KEY]-> DatabaseTable` (with edge attributes for column pair)

---

## 4. Schema Extraction by Database Type

### 4.1 PostgreSQL

- **Catalog**: `information_schema` and `pg_catalog` (e.g. `pg_tables`, `pg_indexes`, `pg_constraint`, `pg_attribute`).
- **Extract**: Tables (with schema), columns (name, data_type, is_nullable, column_default), indexes (name, columns, uniqueness), primary key, foreign keys (referenced table/column).
- **Connection**: Project config `database.connection` (URL or host/port/user/db); read-only user is sufficient.

### 4.2 MySQL / MariaDB

- **Catalog**: `information_schema.tables`, `columns`, `statistics`, `key_column_usage`, `table_constraints`.
- **Extract**: Same logical model (tables, columns, indexes, PK, FK).
- **Connection**: Same pattern as Postgres.

### 4.3 SQLite

- **Catalog**: `sqlite_master`; `PRAGMA table_info(table_name)`; `PRAGMA index_list(table_name)`.
- **Extract**: Tables, columns, indexes; FK from `foreign_key_list` pragma if available.
- **Use case**: Local dev or embedded DBs; same normalized output.

### 4.4 Other (SQL Server, Oracle, etc.)

- **Adapter pattern**: Implement a small adapter per engine that queries that engine’s catalog and emits the normalized model (Database, Table, Column, Index, PK, FK).
- **Optional**: Defer to a third-party library (e.g. SQLAlchemy reflect, or schema inspection tools) and map to our model.

---

## 5. Usage Mapping (Code → Tables)

The **Code Parser** and **Code Indexer** can emit:
- `Function -[:READS]-> Table` / `Function -[:WRITES]-> Table` from SQL strings, ORM calls, or repository patterns.

Data ingestion can **enrich** this:
- **Resolve table names**: If code references `users` and schema has `public.users`, attach to the same `DatabaseTable` node.
- **Validate**: If code references a table that does not exist in the extracted schema, still create a “stub” or tag as unverified.
- **Infer from schema**: If only schema is ingested (no code yet), at least the graph has all tables/columns/indexes; usage edges are added when code is indexed.

**Optional: runtime usage**:
- From **slow query logs** or **audit logs**: Parse query logs to get (service, query, table, columns). Create or update usage edges (e.g. “service X ran SELECT on users”). This is an advanced feature and can be a separate sub-pipeline.

---

## 6. Pipeline Stages

1. **Connect**: Using project config, connect to the database (read-only).
2. **Extract**: Query catalog; build in-memory normalized model (Database, Tables, Columns, Indexes, PKs, FKs).
3. **Deduplicate**: Match to existing SKG nodes by (database_id, schema, table_name) so we do not create duplicates.
4. **SKG write**: Upsert Database, DatabaseTable, Column, Index, PK, FK nodes and edges.
5. **Usage merge**: Merge usage edges from Code Indexer (and optionally from query logs); link Function to DatabaseTable.
6. **Optional vector**: Embed table/column names and descriptions (if any) for semantic search (“tables related to payments”).

---

## 7. Configuration

- **Per-project** (e.g. in manifest):
  - `database.type`: postgres | mysql | sqlite | ...
  - `database.connection`: URL or structured (host, port, user, password, dbname).
  - `database.schema`: optional filter (e.g. only `public` schema in Postgres).
- **Security**: Store credentials in env or secret store; never in plain text in docs. Use a read-only DB user.
- **Frequency**: Full schema extract on init or on schedule (e.g. daily); schema changes are usually infrequent.

---

## 8. Integration with SKG and Context Funnel

- **SKG**: Data ingestion is the **authoritative writer** for Database, DatabaseTable, Column, Index, and their relations. Code Indexer writes Function → READS/WRITES → DatabaseTable.
- **Context Funnel**: When the funnel narrows to a service or endpoint, it can load “tables used by this service” and “columns and indexes of those tables” to explain e.g. “missing index on users.email”.
- **MCP tools**: `get_db_schema(service | db_name)`, `get_db_usage(table_name)` read from the graph and optionally from cached schema snapshots.

---

## 9. Error Handling and Observability

- **Connection failure**: Log and retry with backoff; do not block the rest of the pipeline (e.g. code indexer can still run).
- **Permission**: If catalog access is partial, extract what is possible and log what was skipped.
- **Metrics**: Number of tables/columns/indexes extracted; duration; errors per run.
- **Structured logs**: Database type, connection identifier (no passwords), and success/failure per step.

---

## 10. Summary

Data Ingestion **extracts database metadata** (tables, columns, indexes, PKs, FKs) from live databases and **writes a normalized schema** into the SKG. Together with **usage edges** from the Code Indexer (and optionally from query logs), it allows ContextMint to reason over “which service touches which table” and “what indexes exist,” so the AI can suggest schema-related fixes and explain performance or failure in terms of data access.
