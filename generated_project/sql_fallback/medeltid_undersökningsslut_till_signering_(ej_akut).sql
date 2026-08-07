-- SQL fallback for: Medeltid Undersökningsslut till signering (ej akut)
-- Original DAX: AVERAGEX(KEEPFILTERS(VALUES(...)), AVERAGE(...))
-- Per remissnummer average of undersökningsslut_till_signering_ej_akut,
-- then average of those per-remiss values.

SELECT AVG(avg_per_remiss) AS value
FROM (
    SELECT AVG(undersökningsslut_till_signering_ej_akut) AS avg_per_remiss
    FROM dw_fys_f_undersökning
    GROUP BY remissnummer
) sub
