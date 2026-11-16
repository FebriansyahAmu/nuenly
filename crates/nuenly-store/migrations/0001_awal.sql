-- Setelan aplikasi sebagai JSON per nama, misalnya "config".
CREATE TABLE settings (
    name TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
) STRICT, WITHOUT ROWID;

-- Audit (F-28): kejadian tanpa teks pengguna. `at` dalam milidetik sejak epoch Unix, `event`
-- adalah JSON `AuditEvent`.
CREATE TABLE audit (
    id INTEGER PRIMARY KEY,
    at INTEGER NOT NULL,
    app TEXT,
    mode TEXT NOT NULL,
    event TEXT NOT NULL
) STRICT;

CREATE INDEX audit_at ON audit (at);
