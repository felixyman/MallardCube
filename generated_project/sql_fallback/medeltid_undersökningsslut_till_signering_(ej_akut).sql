-- SQL fallback for: Medeltid Undersökningsslut till signering (ej akut)
-- Original DAX:  AVERAGEX( KEEPFILTERS(VALUES('dw_fys F_Undersökning'[Remissnummer])), CALCULATE(AVERAGE('dw_fys F_Undersökning'[Undersökningsslut till signering - ej akut])) )
--
-- Pattern notes:
--   AVERAGEX — row-level iteration
--   KEEPFILTERS — filter context preservation
--
-- TODO: Implement DuckDB SQL equivalent.
-- Runs via the proxy's direct SQL fallback path.

SELECT 1 AS dummy;
