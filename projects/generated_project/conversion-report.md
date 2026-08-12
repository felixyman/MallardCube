# Conversion Report — SEMANTICMODEL

## Summary

- Fact table: dw_fys F_Undersökning
- Dimensions: 17
- Date-role tables: 7
- Relationships: 16
- Measures: 33 (simple: 22, sql_fallback: 11, manual: 0)
- M-partition tables: 17 (all must be loaded manually)

## Join Map

| Fact Column | Dimension Table | Join Column |
|---|---|---|
| Remissdatum | dw_fys Kalender_Remissdatum | Remissdatum |
| Signeringsdatum | dw_fys Kalender_Signeringsdatum | Signeringsdatum |
| Undersökningsstart | dw_fys Kalender_Undersökningsstart | Undersökningsstart |
| Undersökningsslut | dw_fys Kalender_Undersökningsslut | Undersökningsslut |
| Beställningsdatum | dw_fys Kalender_Beställningsdatum | Beställningsdatum |
| UtförareId | dw_fys D_Utförare | UtförareId |
| PatientId | dw_fys D_Patient | PatientId |
| BeställareId | dw_fys D_Beställare | BeställareId |
| RemisstatusId | dw_fys D_Remisstatus | RemisstatusId |
| ProduktId | dw_fys D_Produkt | ProduktId |
| Bokningsdatum | dw_fys Kalender_Bokningsdatum | Bokningsdatum |
| SignerareId | dw_fys D_Signerare | SignerareId |
| RemissgruppId | dw_fys D_Remissgrupp | RemissgruppId |
| RemisskoderId | dw_fys D_Remisskoder | RemisskoderId |
| Måldatum_från | dw_fys Kalender_Måldatum | Måldatum |
| FakturamottagareId | dw_fys D_Fakturamottagare | FakturamottagareId |

## Simple measures (Malloy)

| Measure | DAX | Malloy |
|---|---|---|
| Andel signerade DVT Under två timmar |  DIVIDE(CALCULATE([Antal signerade DVT-remisser],'dw_fys F_Undersökning'[Beställning till signering]<=0.083333),[Antal signerade DVT-remisser]) | antal_signerade_dvt_remisser { where: beställning_till_signering <= '0.083333' } / antal_signerade_dvt_remisser |
| Andel under fyra timmar |  DIVIDE([Antal remisser akuta inneliggande (Under fyra timmar)],[Antal remisser akuta inneliggande]) | antal_remisser_akuta_inneliggande_(under_fyra_timmar) / antal_remisser_akuta_inneliggande |
| Antal bokade remisser | CALCULATE([Antal remisser],'dw_fys D_Remisstatus'[Remisstatus]="Tid") | antal_remisser { where: remisstatus = 'TID' } |
| Antal obokade remisser | CALCULATE([Antal remisser],'dw_fys D_Remisstatus'[Remisstatus]="Ej tid") | antal_remisser { where: remisstatus = 'EJ TID' } |
| Antal osignerade utförda undersökningar | CALCULATE([Antal undersökningar],'dw_fys F_Undersökning'[Signeringsstatus]="Osignerad",'dw_fys D_Remisskoder'[Undersökningsstatus]="Utförd") | antal_undersökningar { where: signeringsstatus = 'OSIGNERAD' } |
| Antal patienter | DISTINCTCOUNT('dw_fys F_Undersökning'[PatientId]) | patientid.count(distinct true) |
| Antal remisser | DISTINCTCOUNT('dw_fys F_Undersökning'[Remissnummer]) | remissnummer.count(distinct true) |
| Antal remisser akuta inneliggande |  CALCULATE(DISTINCTCOUNT('dw_fys F_Undersökning'[Remissnummer]),'dw_fys F_Undersökning'[Flagga Akuten]=1) | remissnummer.count(distinct true) { where: flagga_akuten = '1' } |
| Antal remisser akuta inneliggande (Under fyra timmar) |  CALCULATE([Antal remisser akuta inneliggande], 'dw_fys F_Undersökning'[Undersökningsslut till signering]<=0.1666667) | antal_remisser_akuta_inneliggande { where: undersökningsslut_till_signering <= '0.1666667' } |
| Antal remisser i urval | CALCULATE([Antal remisser],'dw_fys D_Remisskoder'[Akut] ="Ja",  'dw_fys F_Undersökning'[Beställning till undersökningsstart]<> BLANK()) | antal_remisser { where: akut = 'JA' } |
| Antal remisser som väntat %3e30 dagar |  CALCULATE([Antal obokade remisser], 'dw_fys F_Undersökning'[Remiss till idag] > 30, 'dw_fys F_Undersökning'[Remiss till idag] <=60) | antal_obokade_remisser { where: remiss_till_idag > '30' } |
| Antal remisser som väntat %3e60 dagar |  CALCULATE([Antal obokade remisser], 'dw_fys F_Undersökning'[Remiss till idag] > 60, 'dw_fys F_Undersökning'[Remiss till idag] <= 90) | antal_obokade_remisser { where: remiss_till_idag > '60' } |
| Antal remisser som väntat %3e90 dagar |  CALCULATE([Antal obokade remisser], 'dw_fys F_Undersökning'[Remiss till idag] > 90) | antal_obokade_remisser { where: remiss_till_idag > '90' } |
| Antal remissiser med tid inom 4 veckor | CALCULATE([Antal undersökningar],'dw_fys F_Undersökning'[Remiss till bokning] <= 28) | antal_undersökningar { where: remiss_till_bokning <= '28' } |
| Antal saknade remisser i underlag | CALCULATE([Antal remisser],'dw_fys D_Remisskoder'[Akut] ="Ja",  'dw_fys F_Undersökning'[Beställning till undersökningsstart]= BLANK()) | antal_remisser { where: akut = 'JA' } |
| Antal undersökningar | COUNT('dw_fys F_Undersökning'[UndersökningId]) | undersökningid.count() |
| Antal uteblivna remisser | CALCULATE([Antal remisser],'dw_fys D_Remisskoder'[Utebliven]="Ja") | antal_remisser { where: utebliven = 'JA' } |
| Antal utförda remisser | CALCULATE([Antal remisser],'dw_fys D_Remisskoder'[Undersökningsstatus]="Utförd") | antal_remisser { where: undersökningsstatus = 'UTFÖRD' } |
| Medeltid registrering till signering | AVERAGE('dw_fys F_Undersökning'[Registrering till signering]) | registrering_till_signering.avg() |
| Medeltid undersökningsslut till signering | AVERAGE('dw_fys F_Undersökning'[Undersökningsslut till signering]) | undersökningsslut_till_signering.avg() |
| Referens DVT remiss |  0.8 | 0.8 |
| Tid inom 4 veckor från skickad remiss | CALCULATE([Antal undersökningar],'dw_fys F_Undersökning'[Remiss till bokning] <= 28)/[Antal undersökningar] | antal_undersökningar { where: remiss_till_bokning <= '28' } |

## SQL fallback measures

| Measure | DAX pattern | Fallback file |
|---|---|---|
| Antal signerade DVT-remisser |  SUMMARIZE( FILTER('dw_fys D_Remisskoder',[Akut] ="Ja"), "Antal remisser", CALCULATE([Antal remisser] , 'dw_fys D_Produkt'[ProduktKod] IN {"516" ,  "526", "524"} , 'dw_fys F_Undersökning'[Beställningstimme] IN { 8, 9, 10, 11, 12, 13, 14} , 'dw_fys Kalender_Signeringsdatum'[VeckodagsSiffra] IN { 1,2,3,4,5} , RIGHT('dw_fys D_Beställare'[BeställareKod],3)  IN {"M08"} ) ) | sql_fallback/antal_signerade_dvt_remisser.sql |
| Antal utförda remisser (ack Kvartal) CY | CALCULATE( [Antal utförda remisser], FILTER( ALLSELECTED('dw_fys Kalender_Undersökningsslut'[Kvartal]), ISONORAFTER('dw_fys Kalender_Undersökningsslut'[Kvartal],    MAX('dw_fys Kalender_Undersökningsslut'[Kvartal]), DESC) ),'dw_fys Kalender_Undersökningsslut'[ÅR] = YEAR(TODAY()) ) | sql_fallback/antal_utförda_remisser_(ack_kvartal)_cy.sql |
| Antal utförda remisser (ack Kvartal) CY-1 | CALCULATE( [Antal utförda remisser], FILTER( ALLSELECTED('dw_fys Kalender_Undersökningsslut'[Kvartal]), ISONORAFTER('dw_fys Kalender_Undersökningsslut'[Kvartal],    MAX('dw_fys Kalender_Undersökningsslut'[Kvartal]), DESC) ),'dw_fys Kalender_Undersökningsslut'[ÅR] = YEAR(TODAY())-1 ) | sql_fallback/antal_utförda_remisser_(ack_kvartal)_cy_1.sql |
| Antal utförda remisser (ack Månad) CY-1 | CALCULATE( [Antal utförda remisser], FILTER( ALLSELECTED('dw_fys Kalender_Undersökningsslut'[Månad]), ISONORAFTER('dw_fys Kalender_Undersökningsslut'[Månad],    MAX('dw_fys Kalender_Undersökningsslut'[Månad]), DESC) ),'dw_fys Kalender_Undersökningsslut'[ÅR] = YEAR(TODAY())-1 ) | sql_fallback/antal_utförda_remisser_(ack_månad)_cy_1.sql |
| Antal utförda remisser (ack månad) CY | CALCULATE( [Antal utförda remisser], FILTER( ALLSELECTED('dw_fys Kalender_Undersökningsslut'[Månad]), ISONORAFTER('dw_fys Kalender_Undersökningsslut'[Månad],    MAX('dw_fys Kalender_Undersökningsslut'[Månad]), DESC) ),'dw_fys Kalender_Undersökningsslut'[ÅR] = YEAR(TODAY()) ) | sql_fallback/antal_utförda_remisser_(ack_månad)_cy.sql |
| Antal utförda remisser (ack vecka) CY | CALCULATE( [Antal utförda remisser], FILTER( ALLSELECTED('dw_fys Kalender_Undersökningsslut'[Vecka]), ISONORAFTER('dw_fys Kalender_Undersökningsslut'[Vecka],    MAX('dw_fys Kalender_Undersökningsslut'[Vecka]), DESC) ),'dw_fys Kalender_Undersökningsslut'[ÅR] = YEAR(TODAY()) ) | sql_fallback/antal_utförda_remisser_(ack_vecka)_cy.sql |
| Antal utförda remisser (ack vecka) CY-1 | CALCULATE( [Antal utförda remisser], FILTER( ALLSELECTED('dw_fys Kalender_Undersökningsslut'[Vecka]), ISONORAFTER('dw_fys Kalender_Undersökningsslut'[Vecka],    MAX('dw_fys Kalender_Undersökningsslut'[Vecka]), DESC) ),'dw_fys Kalender_Undersökningsslut'[ÅR] = YEAR(TODAY())-1 ) | sql_fallback/antal_utförda_remisser_(ack_vecka)_cy_1.sql |
| Medeltid Undersökningsslut till signering (ej akut) |  AVERAGEX( KEEPFILTERS(VALUES('dw_fys F_Undersökning'[Remissnummer])), CALCULATE(AVERAGE('dw_fys F_Undersökning'[Undersökningsslut till signering - ej akut])) ) | sql_fallback/medeltid_undersökningsslut_till_signering_(ej_akut).sql |
| Median beställning till undersökning | MEDIAN('dw_fys F_Undersökning'[Beställning till undersökningsstart]) | sql_fallback/median_beställning_till_undersökning.sql |
| Median beställning till undersökning (Akut & timmar) | MEDIAN('dw_fys F_Undersökning'[Beställning till undersökningsstart (Akut & timmar)]) | sql_fallback/median_beställning_till_undersökning_(akut_&_timmar).sql |
| Median beställning till undersökning (Akut) | MEDIAN('dw_fys F_Undersökning'[Beställning till undersökningsstart (Akut)]) | sql_fallback/median_beställning_till_undersökning_(akut).sql |

## Data loading checklist

All tables use M (Power Query) partitions and must be loaded into DuckDB manually.

**Quick start (with date-dimension bootstrap):**

```
duckdb data/f_undersokning.db < bootstrap.sql
```

This creates the schema, seeds a populated `date_dim` calendar table, and
creates the DuckDB schema and seeded `date_dim`. (The `db_path` in `proxy-config.json` already points at this file.) Then load your own data into the
listed tables below.

Run `schema.sql` to create the tables, then load data via:

- DuckDB CLI: `INSERT INTO ... SELECT ... FROM 'source.csv'`
- Or export your SSAS source to Parquet/CSV and import into DuckDB.

### Tables to load

- [ ] `dw_fys_f_undersökning` (fact)
- [ ] `dw_fys_d_beställare` (dimension)
- [ ] `dw_fys_d_fakturamottagare` (dimension)
- [ ] `dw_fys_d_patient` (dimension)
- [ ] `dw_fys_d_produkt` (dimension)
- [ ] `dw_fys_d_remissgrupp` (dimension)
- [ ] `dw_fys_d_remisskoder` (dimension)
- [ ] `dw_fys_d_remisstatus` (dimension)
- [ ] `dw_fys_d_signerare` (dimension)
- [ ] `dw_fys_d_utförare` (dimension)
- [ ] `dw_fys_kalender_beställningsdatum` (date-role)
- [ ] `dw_fys_kalender_bokningsdatum` (date-role)
- [ ] `dw_fys_kalender_måldatum` (date-role)
- [ ] `dw_fys_kalender_remissdatum` (date-role)
- [ ] `dw_fys_kalender_signeringsdatum` (date-role)
- [ ] `dw_fys_kalender_undersökningsslut` (date-role)
- [ ] `dw_fys_kalender_undersökningsstart` (date-role)
- [ ] `carmae` (lookup)

## Roles

Security roles detected but NOT supported by the proxy:

- fys_läsbehörighet: Läsbehörighet för fys-data

Must be enforced outside the proxy if needed.
