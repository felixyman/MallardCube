-- SQL fallback for: Antal signerade DVT-remisser
-- Original DAX:  SUMMARIZE( FILTER('dw_fys D_Remisskoder',[Akut] ="Ja"), "Antal remisser", CALCULATE([Antal remisser] , 'dw_fys D_Produkt'[ProduktKod] IN {"516" ,  "526", "524"} , 'dw_fys F_Undersökning'[Beställningstimme] IN { 8, 9, 10, 11, 12, 13, 14} , 'dw_fys Kalender_Signeringsdatum'[VeckodagsSiffra] IN { 1,2,3,4,5} , RIGHT('dw_fys D_Beställare'[BeställareKod],3)  IN {"M08"} ) )
--
-- Pattern notes:
--   FILTER — context manipulation
--
-- TODO: Implement DuckDB SQL equivalent.
-- Runs via the proxy's direct SQL fallback path.

SELECT 1 AS dummy;
