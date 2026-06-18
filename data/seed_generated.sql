-- Minimal seed data for generated_project smoke test.
-- DuckDB-safe: columns use underscores, no parentheses.

CREATE TABLE IF NOT EXISTS dw_fys_f_undersökning (
    undersökningid BIGINT,
    remissnummer BIGINT,
    registrering_till_signering DOUBLE,
    undersökningsslut_till_signering DOUBLE,
    beställning_till_signering DOUBLE,
    remiss_till_bokning DOUBLE,
    remiss_till_idag BIGINT,
    beställningstimme BIGINT,
    flagga_akuten BIGINT,
    signeringsstatus VARCHAR,
    remisstatusid BIGINT,
    remisskoderid BIGINT,
    remissgruppid BIGINT,
    remisskod VARCHAR,
    signerareid BIGINT,
    signeringsdatum TIMESTAMP,
    utförareid BIGINT,
    patientid BIGINT,
    beställareid BIGINT,
    produktid BIGINT,
    fakturamottagareid BIGINT,
    remissdatum TIMESTAMP,
    beställningsdatum TIMESTAMP,
    bokningsdatum TIMESTAMP,
    undersökningsstart TIMESTAMP,
    undersökningsslut TIMESTAMP,
    måldatum_från TIMESTAMP,
    undersökningsslut_till_signering_ej_akut DOUBLE
);

CREATE TABLE IF NOT EXISTS dw_fys_d_produkt (
    produktid BIGINT,
    bis_kod VARCHAR,
    produkt VARCHAR,
    produktkod VARCHAR
);

CREATE TABLE IF NOT EXISTS dw_fys_d_remisstatus (
    remisstatusid BIGINT,
    remisstatuskod VARCHAR,
    remisstatus VARCHAR
);

CREATE TABLE IF NOT EXISTS dw_fys_d_remisskoder (
    remisskoderid BIGINT,
    remisskoderkod VARCHAR,
    akut VARCHAR,
    utebliven VARCHAR,
    undersökningsstatus VARCHAR
);

CREATE TABLE IF NOT EXISTS dw_fys_d_remissgrupp (
    remissgruppid BIGINT,
    remissgruppkod VARCHAR
);

CREATE TABLE IF NOT EXISTS dw_fys_d_utförare (
    utförareid BIGINT,
    utförarekod VARCHAR
);

CREATE TABLE IF NOT EXISTS dw_fys_d_signerare (
    signerareid BIGINT,
    signerarekod VARCHAR
);

CREATE TABLE IF NOT EXISTS dw_fys_d_patient (
    patientid BIGINT
);

CREATE TABLE IF NOT EXISTS dw_fys_d_beställare (
    beställareid BIGINT,
    beställarekod VARCHAR
);

CREATE TABLE IF NOT EXISTS dw_fys_d_fakturamottagare (
    fakturamottagareid BIGINT,
    fakturamottagarekod VARCHAR
);

CREATE TABLE IF NOT EXISTS dw_fys_kalender_remissdatum (
    remissdatum TIMESTAMP,
    dagnamn VARCHAR,
    veckodagssiffra BIGINT
);

CREATE TABLE IF NOT EXISTS dw_fys_kalender_undersökningsslut (
    undersökningsslut TIMESTAMP,
    dagnamn VARCHAR,
    ÅR BIGINT,
    Kvartal VARCHAR,
    Månad VARCHAR,
    Vecka VARCHAR
);

CREATE TABLE IF NOT EXISTS dw_fys_kalender_signeringsdatum (
    signeringsdatum TIMESTAMP,
    dagnamn VARCHAR,
    veckodagssiffra BIGINT
);

CREATE TABLE IF NOT EXISTS dw_fys_kalender_undersökningsstart (
    undersökningsstart TIMESTAMP,
    dagnamn VARCHAR
);

CREATE TABLE IF NOT EXISTS dw_fys_kalender_beställningsdatum (
    beställningsdatum TIMESTAMP,
    dagnamn VARCHAR
);

CREATE TABLE IF NOT EXISTS dw_fys_kalender_bokningsdatum (
    bokningsdatum TIMESTAMP,
    dagnamn VARCHAR
);

CREATE TABLE IF NOT EXISTS dw_fys_kalender_måldatum (
    måldatum TIMESTAMP,
    dagnamn VARCHAR
);

-- Dimension data
INSERT INTO dw_fys_d_produkt VALUES (1, 'MR_RTG', 'MR/RTG', '516');
INSERT INTO dw_fys_d_produkt VALUES (2, 'ULTRASOUND', 'Ultraljud', '526');
INSERT INTO dw_fys_d_produkt VALUES (3, 'CT', 'Datortomografi', '524');

INSERT INTO dw_fys_d_remisstatus VALUES (1, 'TID', 'Tid');
INSERT INTO dw_fys_d_remisstatus VALUES (2, 'EJ_TID', 'Ej tid');

INSERT INTO dw_fys_d_remisskoder VALUES (1, 'AKUT_JA', 'Ja', 'Nej', 'Utförd');
INSERT INTO dw_fys_d_remisskoder VALUES (2, 'PLANERAD', 'Nej', 'Nej', 'Utförd');

INSERT INTO dw_fys_d_remissgrupp VALUES (1, 'GRUPP_A');
INSERT INTO dw_fys_d_remissgrupp VALUES (2, 'GRUPP_B');

INSERT INTO dw_fys_d_utförare VALUES (1, 'UTF_001');
INSERT INTO dw_fys_d_utförare VALUES (2, 'UTF_002');

INSERT INTO dw_fys_d_signerare VALUES (1, 'SIGN_001');
INSERT INTO dw_fys_d_signerare VALUES (2, 'SIGN_002');

INSERT INTO dw_fys_d_patient VALUES (100);
INSERT INTO dw_fys_d_patient VALUES (200);
INSERT INTO dw_fys_d_patient VALUES (300);

INSERT INTO dw_fys_d_beställare VALUES (1, 'BEST_001');
INSERT INTO dw_fys_d_beställare VALUES (2, 'BEST_002');
INSERT INTO dw_fys_d_beställare VALUES (3, 'XXX_M08');

INSERT INTO dw_fys_d_fakturamottagare VALUES (1, 'FAKT_001');

INSERT INTO dw_fys_kalender_remissdatum VALUES ('2024-01-15'::TIMESTAMP, 'Måndag', 1);
INSERT INTO dw_fys_kalender_remissdatum VALUES ('2024-01-16'::TIMESTAMP, 'Tisdag', 2);
INSERT INTO dw_fys_kalender_remissdatum VALUES ('2024-02-01'::TIMESTAMP, 'Torsdag', 4);
INSERT INTO dw_fys_kalender_remissdatum VALUES ('2024-03-10'::TIMESTAMP, 'Söndag', 7);

INSERT INTO dw_fys_kalender_undersökningsslut VALUES ('2024-01-15'::TIMESTAMP, 'Måndag', 2024, 'Q1', 'Januari', 'V3');
INSERT INTO dw_fys_kalender_undersökningsslut VALUES ('2024-01-16'::TIMESTAMP, 'Tisdag', 2024, 'Q1', 'Januari', 'V3');
INSERT INTO dw_fys_kalender_undersökningsslut VALUES ('2024-02-01'::TIMESTAMP, 'Torsdag', 2024, 'Q1', 'Februari', 'V5');
INSERT INTO dw_fys_kalender_undersökningsslut VALUES ('2024-03-10'::TIMESTAMP, 'Söndag', 2024, 'Q1', 'Mars', 'V10');

INSERT INTO dw_fys_kalender_signeringsdatum VALUES ('2024-01-15'::TIMESTAMP, 'Måndag', 1);
INSERT INTO dw_fys_kalender_signeringsdatum VALUES ('2024-01-16'::TIMESTAMP, 'Tisdag', 2);
INSERT INTO dw_fys_kalender_signeringsdatum VALUES ('2024-02-01'::TIMESTAMP, 'Torsdag', 4);

INSERT INTO dw_fys_kalender_undersökningsstart VALUES ('2024-01-15'::TIMESTAMP, 'Måndag');
INSERT INTO dw_fys_kalender_undersökningsstart VALUES ('2024-01-16'::TIMESTAMP, 'Tisdag');

INSERT INTO dw_fys_kalender_beställningsdatum VALUES ('2024-01-14'::TIMESTAMP, 'Söndag');
INSERT INTO dw_fys_kalender_beställningsdatum VALUES ('2024-01-15'::TIMESTAMP, 'Måndag');

INSERT INTO dw_fys_kalender_bokningsdatum VALUES ('2024-01-14'::TIMESTAMP, 'Söndag');
INSERT INTO dw_fys_kalender_bokningsdatum VALUES ('2024-01-15'::TIMESTAMP, 'Måndag');

INSERT INTO dw_fys_kalender_måldatum VALUES ('2024-01-20'::TIMESTAMP, 'Lördag');
INSERT INTO dw_fys_kalender_måldatum VALUES ('2024-01-21'::TIMESTAMP, 'Söndag');

-- Fact data (10 rows)
INSERT INTO dw_fys_f_undersökning VALUES (1, 1001, 0.5, 0.2, 0.1, 5, 10, 9, 0, 'Signerad', 1, 1, 1, 'RTG_KNÄ', 1, '2024-01-15'::TIMESTAMP, 1, 100, 3, 1, 1, '2024-01-15'::TIMESTAMP, '2024-01-14'::TIMESTAMP, '2024-01-14'::TIMESTAMP, '2024-01-15'::TIMESTAMP, '2024-01-15'::TIMESTAMP, '2024-01-20'::TIMESTAMP, 0.22);
INSERT INTO dw_fys_f_undersökning VALUES (2, 1002, 0.3, 0.1, 0.08, 3, 5, 10, 0, 'Signerad', 1, 1, 2, 'RTG_HAND', 1, '2024-01-15'::TIMESTAMP, 2, 100, 3, 1, 1, '2024-01-15'::TIMESTAMP, '2024-01-14'::TIMESTAMP, '2024-01-14'::TIMESTAMP, '2024-01-15'::TIMESTAMP, '2024-01-15'::TIMESTAMP, '2024-01-20'::TIMESTAMP, 0.12);
INSERT INTO dw_fys_f_undersökning VALUES (3, 1003, 0.7, 0.3, 0.15, 7, 15, 11, 1, 'Osignerad', 2, 2, 1, 'CT_BUK', 2, '2024-01-16'::TIMESTAMP, 1, 200, 1, 2, 1, '2024-01-16'::TIMESTAMP, '2024-01-15'::TIMESTAMP, '2024-01-15'::TIMESTAMP, '2024-01-16'::TIMESTAMP, '2024-01-16'::TIMESTAMP, '2024-01-21'::TIMESTAMP, 0.33);
INSERT INTO dw_fys_f_undersökning VALUES (4, 1004, 0.4, 0.15, 0.09, 4, 8, 12, 1, 'Signerad', 1, 1, 1, 'MR_KNÄ', 1, '2024-01-16'::TIMESTAMP, 2, 300, 3, 2, 1, '2024-01-16'::TIMESTAMP, '2024-01-15'::TIMESTAMP, '2024-01-15'::TIMESTAMP, '2024-01-16'::TIMESTAMP, '2024-01-16'::TIMESTAMP, '2024-01-21'::TIMESTAMP, 0.17);
INSERT INTO dw_fys_f_undersökning VALUES (5, 1005, 0.6, 0.25, 0.12, 6, 12, 13, 0, 'Signerad', 1, 2, 2, 'ULTRASOUND_BUK', 1, '2024-01-17'::TIMESTAMP, 1, 100, 3, 3, 1, '2024-01-17'::TIMESTAMP, '2024-01-16'::TIMESTAMP, '2024-01-16'::TIMESTAMP, '2024-01-17'::TIMESTAMP, '2024-01-17'::TIMESTAMP, '2024-01-21'::TIMESTAMP, 0.27);
INSERT INTO dw_fys_f_undersökning VALUES (6, 1006, 0.8, 0.35, 0.18, 8, 20, 14, 1, 'Osignerad', 2, 1, 1, 'CT_THORAX', 2, '2024-02-01'::TIMESTAMP, 2, 200, 2, 2, 1, '2024-02-01'::TIMESTAMP, '2024-01-31'::TIMESTAMP, '2024-01-31'::TIMESTAMP, '2024-02-01'::TIMESTAMP, '2024-02-01'::TIMESTAMP, '2024-02-05'::TIMESTAMP, 0.37);
INSERT INTO dw_fys_f_undersökning VALUES (7, 1007, 0.2, 0.08, 0.05, 2, 3, 9, 0, 'Signerad', 1, 1, 1, 'RTG_FOT', 1, '2024-02-01'::TIMESTAMP, 1, 100, 3, 1, 1, '2024-02-01'::TIMESTAMP, '2024-01-31'::TIMESTAMP, '2024-01-31'::TIMESTAMP, '2024-02-01'::TIMESTAMP, '2024-02-01'::TIMESTAMP, '2024-02-05'::TIMESTAMP, 0.10);
INSERT INTO dw_fys_f_undersökning VALUES (8, 1001, 0.9, 0.4, 0.2, 10, 25, 15, 2, 'Signerad', 1, 2, 2, 'MR_HJÄRNA', 1, '2024-03-10'::TIMESTAMP, 1, 300, 1, 3, 1, '2024-03-10'::TIMESTAMP, '2024-03-09'::TIMESTAMP, '2024-03-09'::TIMESTAMP, '2024-03-10'::TIMESTAMP, '2024-03-10'::TIMESTAMP, '2024-03-15'::TIMESTAMP, 0.42);
INSERT INTO dw_fys_f_undersökning VALUES (9, 1002, 0.35, 0.12, 0.07, 3, 6, 10, 0, 'Signerad', 1, 1, 1, 'RTG_KNA', 2, '2024-03-10'::TIMESTAMP, 2, 100, 3, 1, 1, '2024-03-10'::TIMESTAMP, '2024-03-09'::TIMESTAMP, '2024-03-09'::TIMESTAMP, '2024-03-10'::TIMESTAMP, '2024-03-10'::TIMESTAMP, '2024-03-15'::TIMESTAMP, 0.14);
INSERT INTO dw_fys_f_undersökning VALUES (10, 1003, 0.45, 0.18, 0.1, 5, 10, 11, 1, 'Osignerad', 2, 2, 2, 'CT_BUK', 1, '2024-03-10'::TIMESTAMP, 1, 200, 2, 2, 1, '2024-03-10'::TIMESTAMP, '2024-03-09'::TIMESTAMP, '2024-03-09'::TIMESTAMP, '2024-03-10'::TIMESTAMP, '2024-03-10'::TIMESTAMP, '2024-03-15'::TIMESTAMP, 0.19);
