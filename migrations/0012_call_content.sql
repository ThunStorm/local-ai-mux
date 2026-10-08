CREATE TABLE call_recording_settings (
    id INTEGER PRIMARY KEY CHECK(id=1),
    enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
    generation INTEGER NOT NULL
);
INSERT INTO call_recording_settings (id,enabled,generation) VALUES (1,0,0);
CREATE TABLE call_contents (
    call_id TEXT PRIMARY KEY NOT NULL REFERENCES call_records(id) ON DELETE CASCADE,
    recording_generation INTEGER NOT NULL,
    request_json TEXT NOT NULL CHECK(json_valid(request_json)),
    response_text TEXT NOT NULL DEFAULT '',
    state TEXT NOT NULL CHECK(state IN ('recording','complete','partial','stopped'))
);
