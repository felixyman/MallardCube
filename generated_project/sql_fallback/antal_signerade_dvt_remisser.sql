-- SQL fallback for: Antal signerade DVT-remisser
-- Original DAX: SUMMARIZE(FILTER(...), CALCULATE(...))
-- Counts distinct remissnummer matching acute DVT product codes,
-- weekday hours, and beställare M08 filter.

SELECT COUNT(DISTINCT f.remissnummer) AS value
FROM dw_fys_f_undersökning f
JOIN dw_fys_d_remisskoder rk ON f.remisskoderid = rk.remisskoderid
JOIN dw_fys_d_produkt p ON f.produktid = p.produktid
JOIN dw_fys_kalender_signeringsdatum kd ON f.signeringsdatum = kd.signeringsdatum
JOIN dw_fys_d_beställare b ON f.beställareid = b.beställareid
WHERE rk.akut = 'Ja'
  AND p.produktkod IN ('516', '526', '524')
  AND f.beställningstimme BETWEEN 8 AND 14
  AND kd.veckodagssiffra BETWEEN 1 AND 5
  AND RIGHT(b.beställarekod, 3) = 'M08'
