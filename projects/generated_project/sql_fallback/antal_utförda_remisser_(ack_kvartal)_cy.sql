-- Cumulative YTD for: Antal utförda remisser (ack Kvartal) CY
-- Original DAX: CALCULATE( [Antal utförda remisser], FILTER( ALLSELECTED('dw_fys Kalender_Undersökningsslut'[Kvartal]), ISONORAFTER('dw_fys Kalender_Undersökningsslut'[Kvartal],    MAX('dw_fys Kalender_Undersökningsslut'[Kvartal]), DESC) ),'dw_fys Kalender_Undersökningsslut'[ÅR] = YEAR(TODAY()) )
-- Calendar: dw_fys_kalender_undersökningsslut, Period: kvartal, Year: år
-- Base measure: Antal utförda remisser
-- Join: f.undersökningsslut = c.undersökningsslut

SELECT
  c.kvartal,
  c.år,
  SUM(base_count) OVER (
    PARTITION BY c.år
    ORDER BY c.kvartal
    ROWS UNBOUNDED PRECEDING
  ) AS ack_value
FROM (
  SELECT
    c.kvartal,
    c.år,
    COUNT(DISTINCT f.remissnummer) AS base_count
  FROM dw_fys_f_undersökning f
  JOIN dw_fys_kalender_undersökningsslut c ON f.undersökningsslut = c.undersökningsslut
  WHERE c.år = EXTRACT(YEAR FROM CURRENT_DATE)
  GROUP BY c.kvartal, c.år
);
