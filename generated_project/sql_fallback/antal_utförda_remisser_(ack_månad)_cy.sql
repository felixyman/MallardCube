-- Cumulative YTD for: Antal utförda remisser (ack månad) CY
-- Original DAX: CALCULATE( [Antal utförda remisser], FILTER( ALLSELECTED('dw_fys Kalender_Undersökningsslut'[Månad]), ISONORAFTER('dw_fys Kalender_Undersökningsslut'[Månad],    MAX('dw_fys Kalender_Undersökningsslut'[Månad]), DESC) ),'dw_fys Kalender_Undersökningsslut'[ÅR] = YEAR(TODAY()) )
-- Calendar: dw_fys_kalender_undersökningsslut, Period: månad, Year: år
-- Base measure: Antal utförda remisser
-- Join: f.undersökningsslut = c.undersökningsslut

SELECT
  c.månad,
  c.år,
  SUM(base_count) OVER (
    PARTITION BY c.år
    ORDER BY c.månad
    ROWS UNBOUNDED PRECEDING
  ) AS ack_value
FROM (
  SELECT
    c.månad,
    c.år,
    COUNT(DISTINCT f.remissnummer) AS base_count
  FROM dw_fys_f_undersökning f
  JOIN dw_fys_kalender_undersökningsslut c ON f.undersökningsslut = c.undersökningsslut
  WHERE c.år = EXTRACT(YEAR FROM CURRENT_DATE)
  GROUP BY c.månad, c.år
);
