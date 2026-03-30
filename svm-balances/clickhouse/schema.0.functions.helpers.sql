-- Split a comma-separated string into Array(String)
-- - Trims whitespace around tokens
-- - Drops empty tokens
CREATE OR REPLACE FUNCTION string_to_array AS (raw) ->
    arrayFilter(x -> x != '',
        arrayMap(x -> trim(x),
            splitByChar(',', ifNull(raw, ''))
        )
    );
