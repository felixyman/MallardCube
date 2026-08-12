-- Cumulative YTD for: Antal utförda remisser (ack vecka) CY-1
-- Original DAX: CALCULATE( [Antal utförda remisser], FILTER( ALLSELECTED('dw_fys Kalender_Undersökningsslut'[Vecka]), ISONORAFTER('dw_fys Kalender_Undersökningsslut'[Vecka],    MAX('dw_fys Kalender_Undersökningsslut'[Vecka]), DESC) ),'dw_fys Kalender_Undersökningsslut'[ÅR] = YEAR(TODAY())-1 )
-- Calendar: dw_fys_kalender_undersökningsslut, Period: vecka, Year: år
-- Base measure: Antal utförda remisser
-- Join: f.undersökningsslut = c.undersökningsslut

SELECT
  c.vecka,
  c.år,
  SUM(base_count) OVER (
    PARTITION BY c.år
    ORDER BY c.vecka
    ROWS UNBOUNDED PRECEDING
  ) AS ack_value
FROM (
  SELECT
    c.vecka,
    c.år,
    COUNT(DISTINCT f.remissnummer) AS base_count
  FROM dw_fys_f_undersökning f
  JOIN dw_fys_kalender_undersökningsslut c ON f.undersökningsslut = c.undersökningsslut
  WHERE c.år = EXTRACT(YEAR FROM CURRENT_DATE) - 1
  GROUP BY c.vecka, c.år
);
