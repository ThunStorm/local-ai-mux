-- Optional, bounded metadata only. Legacy calls deliberately stay NULL.
ALTER TABLE call_records ADD COLUMN request_id TEXT;
ALTER TABLE call_records ADD COLUMN diagnostics TEXT CHECK (diagnostics IS NULL OR length(CAST(diagnostics AS BLOB)) <= 8192);
CREATE INDEX call_records_request_id ON call_records(request_id) WHERE request_id IS NOT NULL;
